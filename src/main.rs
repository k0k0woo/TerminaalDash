pub mod app;
pub mod github;
pub mod news;
pub mod schedule;
pub mod shared;
pub mod stocks;
pub mod ui;
pub mod weather;
pub mod overlay;

use app::{Action, App, AppEvent};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    Terminal,
};
use std::{io, sync::mpsc, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (tx, rx) = mpsc::channel();
    
    // Spawn Background Fetchers
    let tx_w = tx.clone();
    tokio::spawn(async move { stocks::fetch_stock_data(tx_w).await; });
    let tx_t = tx.clone();
    tokio::spawn(async move { weather::fetch_weather_data(tx_t).await; });
    let tx_g = tx.clone();
    tokio::spawn(async move { shared::fetch_time(tx_g).await; });
    let tx_n = tx.clone();
    tokio::spawn(async move { github::fetch_github_data(tx_n).await; });
    let tx_so = tx.clone();
    tokio::spawn(async move { news::fetch_news_data(tx_so).await; });
    let tx_so_loop = tx.clone();
    tokio::spawn(async move { stocks::fetch_stock_overview_data(tx_so_loop).await; });

    let mut app = App::new(tx.clone());
    let res = run_app(&mut terminal, &mut app, rx);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    if let Err(err) = res {
        println!("{:?}", err);
    }
    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App, rx: mpsc::Receiver<AppEvent>) -> io::Result<()> {
    let (action_tx, action_rx) = mpsc::channel();

    let tx_input = action_tx.clone();
    std::thread::spawn(move || loop {
        if event::poll(Duration::from_millis(16)).unwrap_or(false) {
            if let Ok(evt) = event::read() {
                if tx_input.send(Action::Input(evt)).is_err() { break; }
            }
        }
    });

    let tx_tick = action_tx.clone();
    std::thread::spawn(move || loop {
        if tx_tick.send(Action::Tick).is_err() { break; }
        std::thread::sleep(Duration::from_millis(50));
    });

    let tx_backend = action_tx.clone();
    std::thread::spawn(move || {
        while let Ok(evt) = rx.recv() {
            if tx_backend.send(Action::Backend(evt)).is_err() { break; }
        }
    });

    loop {
        terminal.draw(|f| ui::ui(f, app));

        let Ok(action) = action_rx.recv() else { break };
        let mut should_quit = app.handle_action(action);

        while let Ok(pending_action) = action_rx.try_recv() {
            if app.handle_action(pending_action) { should_quit = true; }
        }

        if should_quit { return Ok(()); }
    }
    Ok(())
}