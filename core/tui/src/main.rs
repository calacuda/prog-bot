use anyhow::Result;
use app_state::AppState;
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
use std::{
    io::{stdout, Stdout},
    ops::Deref,
    sync::{atomic::AtomicBool, Arc},
};
use tokio::sync::Mutex;

mod app_state;

#[tokio::main]
async fn main() -> Result<()> {
    // println!("Hello, world!");

    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;
    let res = main_loop(&mut terminal);

    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;

    if let Err(e) = res.await {
        eprintln!("drawing to term screen failed: {e}");
    }

    Ok(())
}

async fn main_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    let running = Arc::new(Mutex::new(true));
    let r = running.clone();

    let mut state = AppState::new(r).await?;

    while *running.lock().await {
        terminal.draw(|frame| ui(frame, &mut state))?;

        if event::poll(std::time::Duration::from_millis(16))? {
            if let event::Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                    break;
                } else if key.kind == KeyEventKind::Press
                    && key.code == KeyCode::Esc
                    && state.in_body
                {
                    state.in_body = false;
                } else if key.kind == KeyEventKind::Press
                    && key.code == KeyCode::Enter
                    && !state.in_body
                {
                    state.in_body = true;
                } else if key.kind == KeyEventKind::Press
                    && key.code == KeyCode::Left
                    && !state.in_body
                {
                    state.cycle_left();
                } else if key.kind == KeyEventKind::Press
                    && key.code == KeyCode::Right
                    && !state.in_body
                {
                    state.cycle_right();
                } else if key.kind == KeyEventKind::Press
                    && key.code == KeyCode::Up
                    && state.in_body
                {
                    state.up();
                } else if key.kind == KeyEventKind::Press
                    && key.code == KeyCode::Down
                    && state.in_body
                {
                    state.down();
                }
            }
        }
    }

    Ok(())
}

fn ui(frame: &mut Frame, state: &mut AppState) {
    let area = frame.size();
    frame.render_widget(
        Paragraph::new("Hello Ratatui! (press 'q' to quit)")
            .white()
            .on_blue(),
        area,
    );
}
