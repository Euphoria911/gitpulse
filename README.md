# gitpulse

An interactive terminal dashboard for any git repository. Commit heatmap, top authors, hot files, scrollable commit list, all in a single-binary TUI.

```
$ gitpulse ~/code/some-repo
```

![status](https://img.shields.io/badge/status-stable-brightgreen) ![license](https://img.shields.io/badge/license-MIT-blue)

## Why

Most git visualizations live in heavyweight GUIs. `gitpulse` is just a terminal: launch it inside any repo, press a number to switch views, press `q` to leave. No config, no server, no index to build. One static binary, works on Linux / macOS / Windows.

- **Overview**: branch, HEAD, remote, commit/author/file totals, repo age.
- **Commits**: scrollable list — sha, date, author, subject.
- **Authors**: leaderboard with commit-share bars.
- **Heatmap**: GitHub-style 7×53 grid of commits per day for the last year.
- **Files**: top 50 most-touched files with touch-share bars.

## Install

```bash
# From source
cargo install --git https://github.com/Euphoria911/gitpulse

# Or grab a prebuilt binary from the releases page
# https://github.com/Euphoria911/gitpulse/releases
```

## Usage

```bash
gitpulse              # current directory
gitpulse path/to/repo
```

## Keybindings

| Key | Action |
| --- | --- |
| `1` – `5` | Switch tabs |
| `Tab` / `Shift+Tab` | Next / previous tab |
| `↑` `↓` or `j` `k` | Scroll |
| `PgUp` / `PgDn` | Page scroll |
| `Home` / `End` or `g` / `G` | Jump to top / bottom |
| `?` | Toggle help |
| `q` or `Esc` | Quit |

## How it works

- Reads `.git` directly via `libgit2` (no shelling out to `git`).
- Walks the commit history once at startup, computing all stats in a single pass.
- UI driven by `ratatui` + `crossterm` — works in any ANSI terminal, including Windows Terminal.
- Heatmap color scale is derived from the busiest day in the window — the brightest green is always your peak.

## Caveats

- First-time launch on a large monorepo may take a few seconds (one revwalk + per-commit tree diff).
- Only the first parent is followed for diff-based stats; merge commits are counted as a single touch per changed file against their first parent.
- Author merging is by lowercased email — if the same person has used mismatched emails, they'll show as separate rows.

## Development

```bash
cargo check
cargo test                  # unit + integration
cargo build --release
cargo clippy --all-targets  # lints
```

## License

MIT
