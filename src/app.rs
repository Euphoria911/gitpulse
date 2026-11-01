use crate::repo::Stats;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Overview,
    Commits,
    Authors,
    Heatmap,
    Files,
}

impl Tab {
    pub const ALL: [Tab; 5] = [
        Tab::Overview,
        Tab::Commits,
        Tab::Authors,
        Tab::Heatmap,
        Tab::Files,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Commits => "Commits",
            Tab::Authors => "Authors",
            Tab::Heatmap => "Heatmap",
            Tab::Files => "Files",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    pub fn next(self) -> Tab {
        let i = (self.index() + 1) % Self::ALL.len();
        Self::ALL[i]
    }

    pub fn prev(self) -> Tab {
        let i = (self.index() + Self::ALL.len() - 1) % Self::ALL.len();
        Self::ALL[i]
    }
}

pub struct App {
    pub stats: Stats,
    pub tab: Tab,
    pub commits_scroll: usize,
    pub authors_scroll: usize,
    pub files_scroll: usize,
    pub show_help: bool,
    pub should_quit: bool,
}

impl App {
    pub fn new(stats: Stats) -> Self {
        Self {
            stats,
            tab: Tab::Overview,
            commits_scroll: 0,
            authors_scroll: 0,
            files_scroll: 0,
            show_help: false,
            should_quit: false,
        }
    }

    pub fn scroll_down(&mut self, step: usize) {
        let (scroll, max) = self.active_scroll();
        let new_scroll = (scroll + step).min(max.saturating_sub(1));
        self.set_active_scroll(new_scroll);
    }

    pub fn scroll_up(&mut self, step: usize) {
        let (scroll, _) = self.active_scroll();
        let new_scroll = scroll.saturating_sub(step);
        self.set_active_scroll(new_scroll);
    }

    pub fn scroll_home(&mut self) {
        self.set_active_scroll(0);
    }

    pub fn scroll_end(&mut self) {
        let (_, max) = self.active_scroll();
        self.set_active_scroll(max.saturating_sub(1));
    }

    fn active_scroll(&self) -> (usize, usize) {
        match self.tab {
            Tab::Commits => (self.commits_scroll, self.stats.commits.len()),
            Tab::Authors => (self.authors_scroll, self.stats.authors.len()),
            Tab::Files => (self.files_scroll, self.stats.hot_files.len()),
            _ => (0, 0),
        }
    }

    fn set_active_scroll(&mut self, v: usize) {
        match self.tab {
            Tab::Commits => self.commits_scroll = v,
            Tab::Authors => self.authors_scroll = v,
            Tab::Files => self.files_scroll = v,
            _ => {}
        }
    }
}
