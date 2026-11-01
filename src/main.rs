use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, Stdout};
use std::path::PathBuf;
use std::process;
use std::time::Duration;

use app::{App, Tab};
use gitpulse::{app, repo, ui};

/// An interactive terminal dashboard for a git repository.
#[derive(Parser, Debug)]
#[command(name = "gitpulse", version, about, long_about = None)]
struct Cli {
    /// Path to the git repository (defaults to the current directory).
    #[arg(default_value = ".")]
    path: PathBuf,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("gitpulse: error: {:#}", e);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = Cli::parse();
    let stats = repo::compute(&args.path).context("computing repo stats")?;
    let app = App::new(stats);

    let mut terminal = setup_terminal().context("initializing terminal")?;
    let result = event_loop(&mut terminal, app);
    restore_terminal(&mut terminal).context("restoring terminal")?;
    result
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn event_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, mut app: App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                handle_key(&mut app, key.code, key.modifiers);
            }
        }
        if app.should_quit {
            return Ok(());
        }
    }
}

fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    if app.show_help {
        app.show_help = false;
        return;
    }
    match code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => app.should_quit = true,
        KeyCode::Char('?') => app.show_help = true,
        KeyCode::Char('1') => app.tab = Tab::Overview,
        KeyCode::Char('2') => app.tab = Tab::Commits,
        KeyCode::Char('3') => app.tab = Tab::Authors,
        KeyCode::Char('4') => app.tab = Tab::Heatmap,
        KeyCode::Char('5') => app.tab = Tab::Files,
        KeyCode::Tab => app.tab = app.tab.next(),
        KeyCode::BackTab => app.tab = app.tab.prev(),
        KeyCode::Down | KeyCode::Char('j') => app.scroll_down(1),
        KeyCode::Up | KeyCode::Char('k') => app.scroll_up(1),
        KeyCode::PageDown => app.scroll_down(10),
        KeyCode::PageUp => app.scroll_up(10),
        KeyCode::Home | KeyCode::Char('g') => app.scroll_home(),
        KeyCode::End | KeyCode::Char('G') => app.scroll_end(),
        _ => {}
    }
}
