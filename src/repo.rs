use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Utc};
use git2::{Commit, Repository, Sort};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Everything we display in the TUI, computed once at startup.
pub struct Stats {
    pub repo_path: PathBuf,
    pub head_branch: String,
    pub head_short: String,
    pub remote_url: Option<String>,

    pub total_commits: usize,
    pub total_authors: usize,
    pub total_files: usize,
    pub first_commit_date: Option<DateTime<Local>>,
    pub last_commit_date: Option<DateTime<Local>>,

    pub commits: Vec<CommitSummary>,
    pub authors: Vec<AuthorStats>,
    pub heatmap: Vec<DayBucket>,
    pub hot_files: Vec<FileStats>,
}

#[derive(Debug, Clone)]
pub struct CommitSummary {
    pub short_sha: String,
    pub author: String,
    pub date: DateTime<Local>,
    pub subject: String,
}

#[derive(Debug, Clone)]
pub struct AuthorStats {
    pub name: String,
    pub email: String,
    pub commits: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct DayBucket {
    pub date: NaiveDate,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct FileStats {
    pub path: String,
    pub touches: usize,
}

pub fn compute(path: &Path) -> Result<Stats> {
    let repo = Repository::discover(path)
        .with_context(|| format!("no git repository found at or above {}", path.display()))?;
    let repo_path = repo.workdir().unwrap_or_else(|| repo.path()).to_path_buf();

    let head_ref = repo.head().context("reading HEAD")?;
    let head_branch = head_ref.shorthand().unwrap_or("(detached)").to_string();
    let head_short = head_ref
        .target()
        .map(|oid| oid.to_string()[..7].to_string())
        .unwrap_or_else(|| "???????".into());

    let remote_url = repo
        .find_remote("origin")
        .ok()
        .and_then(|r| r.url().map(|s| s.to_string()));

    let total_files = count_files_in_head(&repo)?;

    let (commits, authors_map, heatmap_map, hot_files_map, first_date, last_date) =
        walk_history(&repo)?;

    let total_commits = commits.len();
    let total_authors = authors_map.len();

    let mut authors: Vec<AuthorStats> = authors_map.into_values().collect();
    authors.sort_by(|a, b| b.commits.cmp(&a.commits).then(a.name.cmp(&b.name)));

    let heatmap = build_heatmap(&heatmap_map);

    let mut hot_files: Vec<FileStats> = hot_files_map
        .into_iter()
        .map(|(path, touches)| FileStats { path, touches })
        .collect();
    hot_files.sort_by(|a, b| b.touches.cmp(&a.touches).then(a.path.cmp(&b.path)));
    hot_files.truncate(50);

    Ok(Stats {
        repo_path,
        head_branch,
        head_short,
        remote_url,
        total_commits,
        total_authors,
        total_files,
        first_commit_date: first_date,
        last_commit_date: last_date,
        commits,
        authors,
        heatmap,
        hot_files,
    })
}

fn count_files_in_head(repo: &Repository) -> Result<usize> {
    let Ok(head) = repo.head() else { return Ok(0) };
    let Ok(commit) = head.peel_to_commit() else {
        return Ok(0);
    };
    let tree = commit.tree().context("reading HEAD tree")?;
    let mut count = 0usize;
    tree.walk(git2::TreeWalkMode::PreOrder, |_root, entry| {
        if entry.kind() == Some(git2::ObjectType::Blob) {
            count += 1;
        }
        git2::TreeWalkResult::Ok
    })?;
    Ok(count)
}

type WalkOut = (
    Vec<CommitSummary>,
    HashMap<String, AuthorStats>,
    HashMap<NaiveDate, usize>,
    HashMap<String, usize>,
    Option<DateTime<Local>>,
    Option<DateTime<Local>>,
);

fn walk_history(repo: &Repository) -> Result<WalkOut> {
    let mut revwalk = repo.revwalk().context("creating revwalk")?;
    revwalk.set_sorting(Sort::TIME)?;
    revwalk.push_head()?;

    let mut commits = Vec::new();
    let mut authors_map: HashMap<String, AuthorStats> = HashMap::new();
    let mut heatmap_map: HashMap<NaiveDate, usize> = HashMap::new();
    let mut hot_files_map: HashMap<String, usize> = HashMap::new();

    let mut first: Option<DateTime<Local>> = None;
    let mut last: Option<DateTime<Local>> = None;

    for oid_result in revwalk {
        let oid = oid_result?;
        let commit = repo.find_commit(oid)?;
        let date = commit_time_to_local(&commit);
        if last.is_none() {
            last = Some(date);
        }
        first = Some(date);

        let author = commit.author();
        let name = author.name().unwrap_or("(unknown)").to_string();
        let email = author.email().unwrap_or("unknown@example.com").to_string();

        let author_key = email.to_lowercase();
        authors_map
            .entry(author_key)
            .and_modify(|a| a.commits += 1)
            .or_insert(AuthorStats {
                name: name.clone(),
                email: email.clone(),
                commits: 1,
            });

        *heatmap_map.entry(date.date_naive()).or_insert(0) += 1;

        for path in files_touched_by(repo, &commit)? {
            *hot_files_map.entry(path).or_insert(0) += 1;
        }

        commits.push(CommitSummary {
            short_sha: oid.to_string()[..7].to_string(),
            author: name,
            date,
            subject: commit.summary().unwrap_or("(no message)").to_string(),
        });
    }

    Ok((
        commits,
        authors_map,
        heatmap_map,
        hot_files_map,
        first,
        last,
    ))
}

fn files_touched_by(repo: &Repository, commit: &Commit) -> Result<Vec<String>> {
    let tree = commit.tree()?;
    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };

    let mut opts = git2::DiffOptions::new();
    opts.include_typechange(true);
    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut opts))?;

    let mut out: Vec<String> = Vec::new();
    diff.foreach(
        &mut |delta, _| {
            if let Some(path) = delta.new_file().path().or_else(|| delta.old_file().path()) {
                out.push(path.to_string_lossy().into_owned());
            }
            true
        },
        None,
        None,
        None,
    )?;
    Ok(out)
}

fn commit_time_to_local(commit: &Commit) -> DateTime<Local> {
    let t = commit.time();
    let secs = t.seconds();
    let utc = Utc.timestamp_opt(secs, 0).single().unwrap_or_else(Utc::now);
    utc.with_timezone(&Local)
}

/// Build a dense array of day buckets from today back 364 days (365 days total, inclusive),
/// filling in zeros where no commits landed.
fn build_heatmap(counts: &HashMap<NaiveDate, usize>) -> Vec<DayBucket> {
    let today = Local::now().date_naive();
    let start = today - Duration::days(364);
    let mut out = Vec::with_capacity(365);
    let mut d = start;
    while d <= today {
        let count = counts.get(&d).copied().unwrap_or(0);
        out.push(DayBucket { date: d, count });
        d = d.succ_opt().unwrap_or(d);
    }
    out
}

/// Helper for the UI: group heatmap into weeks (columns), each week has 7 days (rows, Sun→Sat).
/// The returned grid is `[week][weekday_index]`, where weekday_index 0 = Sunday.
pub fn heatmap_grid(heatmap: &[DayBucket]) -> Vec<Vec<Option<DayBucket>>> {
    if heatmap.is_empty() {
        return Vec::new();
    }
    // Pad the start so the first column begins on Sunday.
    let first_dow = heatmap[0].date.weekday().num_days_from_sunday() as usize;
    let mut flat: Vec<Option<DayBucket>> = Vec::with_capacity(heatmap.len() + first_dow);
    for _ in 0..first_dow {
        flat.push(None);
    }
    flat.extend(heatmap.iter().map(|d| Some(*d)));

    // Chunk into weeks of 7.
    let mut grid: Vec<Vec<Option<DayBucket>>> = Vec::new();
    for chunk in flat.chunks(7) {
        let mut col: Vec<Option<DayBucket>> = chunk.to_vec();
        while col.len() < 7 {
            col.push(None);
        }
        grid.push(col);
    }
    grid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heatmap_grid_has_sunday_padding() {
        // If the start date is a Wednesday, the first column should have 3 Nones (Sun, Mon, Tue)
        // then the day.
        let wed = NaiveDate::from_ymd_opt(2026, 10, 28).unwrap(); // Wednesday
        let heatmap = vec![DayBucket {
            date: wed,
            count: 1,
        }];
        let grid = heatmap_grid(&heatmap);
        assert_eq!(grid.len(), 1);
        assert!(grid[0][0].is_none());
        assert!(grid[0][1].is_none());
        assert!(grid[0][2].is_none());
        assert_eq!(grid[0][3].map(|b| b.count), Some(1));
    }

    #[test]
    fn heatmap_grid_handles_empty() {
        let grid = heatmap_grid(&[]);
        assert!(grid.is_empty());
    }
}
