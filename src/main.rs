use std::{io, time::Duration, error::Error};
use ratatui::{Terminal, backend::CrosstermBackend, widgets::{Block, Borders, Paragraph}, layout::{Layout, Constraint, Direction}};
use crossterm::{execute, terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}, event::{self, Event, KeyCode}};
use yahoo_finance_api as yahoo;

struct AppState {
    stock_data: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let mut app_state = AppState {
        stock_data: "Loading...".to_string(),
    };

    // Provider init
    let provider = yahoo::YahooConnector::new()?;

    // Define tickers
    let tickers = ["TSLA", "2330.TW"];
    let mut display_text = String::new();

    // Fetch data for each ticker (Separate requests as API returns historical candles per request)
    for symbol in tickers {
        match provider.get_latest_quotes(symbol, "1d").await {
            Ok(response) => {
                // Get the latest quote (last candle in the history)
                if let Ok(quote) = response.last_quote() {
                    // quote.close is the price
                    display_text.push_str(&format!("{}: ${:.2}\n", symbol, quote.close));
                } else {
                    display_text.push_str(&format!("{}: No Data\n", symbol));
                }
            },
            Err(e) => {
                display_text.push_str(&format!("{}: Error {}\n", symbol, e));
            }
        }
    }
    app_state.stock_data = display_text;

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(10), Constraint::Percentage(90)].as_ref())
                .split(f.area());

            let title = Paragraph::new("Rust TUI Investment Dashboard (v0.1)")
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(title, chunks[0]);

            let content = Paragraph::new(app_state.stock_data.as_str())
                .block(Block::default().title("Live Quotes").borders(Borders::ALL));
            f.render_widget(content, chunks[1]);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if let KeyCode::Char('q') = key.code {
                    break;
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
