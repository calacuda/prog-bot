use anyhow::Result;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, KeyCode, KeyEventKind},
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        ExecutableCommand,
    },
    style::Stylize,
    widgets::Paragraph,
    Frame, Terminal,
};
use std::io::{stdout, Stdout};

mod app_state;

fn main() -> Result<()> {
    // println!("Hello, world!");

    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;
    let res = main_loop(&mut terminal);

    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;

    if let Err(e) = res {
        eprintln!("drawing to term screen failed: {e}");
    }

    Ok(())
}

fn main_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    let mut state = AppState::new();

    loop {
        terminal.draw(|frame| ui(frame, state))?;

        if event::poll(std::time::Duration::from_millis(16))? {
            if let event::Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }

    Ok(())
}

fn ui(frame: &mut Frame, state: AppState) -> Result<()> {
    let area = frame.size();
    frame.render_widget(
        // Paragraph::new("Hello Ratatui! (press 'q' to quit)")
        //     .white()
        //     .on_blue(),
        area,
    );

    Ok(())
}
