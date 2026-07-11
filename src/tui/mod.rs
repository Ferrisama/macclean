mod app;
mod ui;

use std::io::{self, Stdout};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

pub use app::{Action, App};

type Term = Terminal<CrosstermBackend<Stdout>>;

fn setup_terminal() -> Result<Term> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
}

/// Launch the interactive dashboard. All existing `macclean <command>`
/// subcommands remain available and unaffected for scripting -- this is
/// only what runs when `macclean` is invoked with no subcommand.
pub fn run(dry_run: bool, yes: bool) -> Result<()> {
    // Make sure a panic mid-render doesn't leave the user's terminal in
    // raw/alternate-screen mode with no way to see what happened.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));

    let mut terminal = setup_terminal()?;
    let mut app = App::new(dry_run, yes);
    let result = event_loop(&mut terminal, &mut app);

    restore_terminal();
    let _ = std::panic::take_hook();
    result
}

fn event_loop(terminal: &mut Term, app: &mut App) -> Result<()> {
    app.refresh_dashboard();

    loop {
        // Pick up anything background threads (analyze/scan/build-plan/
        // execute) have finished since the last tick, whether or not a
        // key/mouse event arrives this iteration -- this is what lets size
        // numbers and scan results appear live without ever blocking input.
        app.poll_bg();
        terminal.draw(|frame| ui::draw(frame, app))?;

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }

        let action = match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => app.on_key(key),
            Event::Mouse(mouse) if matches!(mouse.kind, MouseEventKind::Down(_)) => {
                app.on_click(mouse.column, mouse.row)
            }
            _ => Action::None,
        };

        if let Action::RunClean = action {
            // Leave the TUI's screen entirely and run cleaners through their
            // normal, already-tested CLI code path (which prints directly to
            // stdout) rather than trying to capture that output into a
            // widget. This one is intentionally foreground/blocking -- the
            // user is meant to watch it happen.
            restore_terminal();
            app.run_selected_clean();
            *terminal = setup_terminal()?;
            terminal.clear()?;
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
