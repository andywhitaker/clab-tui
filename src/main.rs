use clab_tui::app::App;
use clab_tui::clab::parser::TopologyParser;
use clab_tui::event::EventHandler;
use clap::Parser;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{stdout, Result};
use std::panic;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "clab-tui")]
#[command(author = "Containerlab TUI Team")]
#[command(version)]
#[command(about = "Terminal UI & 2D Network Canvas for Containerlab", long_about = None)]
pub struct CliArgs {
    /// Path to an existing Containerlab topology file (*.clab.yml)
    #[arg(short, long)]
    pub file: Option<PathBuf>,

    /// Run in offline mock mode (no Docker or root required)
    #[arg(short, long)]
    pub mock: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = CliArgs::parse();

    // Load topology if provided
    let (topo, path) = if let Some(ref p) = args.file {
        match TopologyParser::parse_file(p) {
            Ok(t) => (Some(t), Some(p.clone())),
            Err(e) => {
                eprintln!("Error loading topology file '{}': {}", p.display(), e);
                std::process::exit(1);
            }
        }
    } else {
        (None, None)
    };

    // Install panic hook to restore terminal cleanly on crash
    install_panic_hook();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Create App instance
    let mut app = App::new(topo, path, args.mock);

    // Run event loop
    let events = EventHandler::new(Duration::from_millis(50));
    let res = run_loop(&mut terminal, &mut app, events).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Application error: {:?}", err);
    }

    Ok(())
}

async fn run_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    mut events: EventHandler,
) -> Result<()> {
    while app.running {
        terminal
            .draw(|frame| app.render(frame))
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        if let Some(event) = events.rx.recv().await {
            app.handle_event(event, &events.tx);
        }
    }
    Ok(())
}

fn install_panic_hook() {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic_info);
    }));
}
