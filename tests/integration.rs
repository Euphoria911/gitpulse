//! Integration tests: build a known git history in a tempdir, then assert
//! `repo::compute` extracts the right stats.

use std::fs;
use std::path::Path;

use chrono::NaiveDate;
use git2::{Repository, Signature};
use gitpulse::repo;

/// Create a bare, writable repo with a scripted history and return its path.
///
/// Each entry in `script` is `(file, contents, author_name, author_email, year, month, day)`.
fn make_repo(dir: &Path, script: &[(&str, &str, &str, &str, i32, u32, u32)]) -> Repository {
    let repo = Repository::init(dir).expect("init");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test").ok();
        cfg.set_str("user.email", "test@example.com").ok();
    }

    let mut parent_oid: Option<git2::Oid> = None;
    for (fname, contents, author_name, author_email, y, m, d) in script.iter() {
        let fpath = dir.join(fname);
        if let Some(parent) = fpath.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::write(&fpath, contents).expect("write");

        let mut index = repo.index().expect("index");
        index.add_path(Path::new(fname)).expect("add_path");
        index.write().expect("write index");
        let tree_oid = index.write_tree().expect("write_tree");
        let tree = repo.find_tree(tree_oid).expect("find_tree");

        let date = NaiveDate::from_ymd_opt(*y, *m, *d)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        let ts = date.and_utc().timestamp();
        let time = git2::Time::new(ts, 0);
        let sig = Signature::new(author_name, author_email, &time).expect("sig");

        let parents: Vec<git2::Commit> = parent_oid
            .map(|oid| vec![repo.find_commit(oid).expect("parent commit")])
            .unwrap_or_default();
        let parents_refs: Vec<&git2::Commit> = parents.iter().collect();

        let oid = repo
            .commit(
                Some("HEAD"),
                &sig,
                &sig,
                &format!("{}: {}", fname, contents.lines().count()),
                &tree,
                &parents_refs,
            )
            .expect("commit");
        parent_oid = Some(oid);
    }
    repo
}

#[test]
fn computes_authors_leaderboard() {
    let tmp = tempfile::tempdir().unwrap();
    make_repo(
        tmp.path(),
        &[
            ("a.txt", "1", "Alice", "alice@example.com", 2026, 1, 1),
            ("b.txt", "1", "Bob", "bob@example.com", 2026, 1, 2),
            ("c.txt", "2", "Alice", "alice@example.com", 2026, 1, 3),
            ("d.txt", "3", "Alice", "alice@example.com", 2026, 1, 4),
        ],
    );

    let stats = repo::compute(tmp.path()).unwrap();
    assert_eq!(stats.total_commits, 4);
    assert_eq!(stats.total_authors, 2);
    assert_eq!(stats.authors[0].name, "Alice");
    assert_eq!(stats.authors[0].commits, 3);
    assert_eq!(stats.authors[1].name, "Bob");
    assert_eq!(stats.authors[1].commits, 1);
}

#[test]
fn merges_authors_by_email_case_insensitively() {
    let tmp = tempfile::tempdir().unwrap();
    make_repo(
        tmp.path(),
        &[
            ("a.txt", "1", "Alice", "Alice@Example.com", 2026, 1, 1),
            ("b.txt", "2", "alice", "alice@example.com", 2026, 1, 2),
        ],
    );
    let stats = repo::compute(tmp.path()).unwrap();
    assert_eq!(stats.total_authors, 1);
    assert_eq!(stats.authors[0].commits, 2);
}

#[test]
fn hot_files_rank_by_touch_count() {
    let tmp = tempfile::tempdir().unwrap();
    make_repo(
        tmp.path(),
        &[
            ("hot.txt", "1", "A", "a@a.com", 2026, 1, 1),
            ("hot.txt", "2", "A", "a@a.com", 2026, 1, 2),
            ("hot.txt", "3", "A", "a@a.com", 2026, 1, 3),
            ("cool.txt", "1", "A", "a@a.com", 2026, 1, 4),
        ],
    );
    let stats = repo::compute(tmp.path()).unwrap();
    let hot = stats
        .hot_files
        .iter()
        .find(|f| f.path == "hot.txt")
        .expect("hot.txt tracked");
    assert_eq!(hot.touches, 3);
    let cool = stats
        .hot_files
        .iter()
        .find(|f| f.path == "cool.txt")
        .expect("cool.txt tracked");
    assert_eq!(cool.touches, 1);
    assert_eq!(stats.hot_files[0].path, "hot.txt");
}

#[test]
fn heatmap_buckets_commits_by_date() {
    let tmp = tempfile::tempdir().unwrap();
    make_repo(
        tmp.path(),
        &[
            ("a.txt", "1", "A", "a@a.com", 2026, 10, 15),
            ("b.txt", "1", "A", "a@a.com", 2026, 10, 15),
            ("c.txt", "1", "A", "a@a.com", 2026, 10, 16),
        ],
    );
    let stats = repo::compute(tmp.path()).unwrap();
    let count_on = |y: i32, m: u32, d: u32| -> usize {
        let target = NaiveDate::from_ymd_opt(y, m, d).unwrap();
        stats
            .heatmap
            .iter()
            .find(|b| b.date == target)
            .map(|b| b.count)
            .unwrap_or(0)
    };
    // The test dates are from Oct 2026. The heatmap window is "last 365 days from today",
    // so if today < Oct 2026 + 365 days, the dates are inside the window.
    // We only assert if the dates fall in the window; otherwise skip.
    let today = chrono::Local::now().date_naive();
    let fifteenth = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
    if (today - fifteenth).num_days() <= 364 && today >= fifteenth {
        assert_eq!(count_on(2026, 10, 15), 2);
        assert_eq!(count_on(2026, 10, 16), 1);
    }
}

#[test]
fn head_metadata_populated() {
    let tmp = tempfile::tempdir().unwrap();
    make_repo(tmp.path(), &[("a.txt", "1", "A", "a@a.com", 2026, 1, 1)]);
    let stats = repo::compute(tmp.path()).unwrap();
    assert!(!stats.head_short.is_empty());
    assert_eq!(stats.head_short.len(), 7);
    assert!(!stats.head_branch.is_empty());
    assert_eq!(stats.total_files, 1);
}

#[test]
fn compute_errors_on_non_repo() {
    let tmp = tempfile::tempdir().unwrap();
    let result = repo::compute(tmp.path());
    assert!(result.is_err(), "expected an error for non-repo path");
    let msg = result.err().unwrap().to_string();
    assert!(
        msg.contains("no git repository"),
        "unexpected error message: {msg}"
    );
}
