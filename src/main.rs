use std::{io, time::Duration, error::Error, fs};
use ratatui::{Terminal, backend::CrosstermBackend, widgets::{Block, Borders, Paragraph}, layout::{Layout, Constraint, Direction}, style::{Style, Color}};
use crossterm::{execute, terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}, event::{self, Event, KeyCode}};
use yahoo_finance_api as yahoo;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone)]
struct Config {
    tickers: Vec<String>,
}

enum InputMode {
    Normal,
    Editing,
}

struct AppState {
    stock_data: String,
    input: String,
    input_mode: InputMode,
    config: Config,
}

impl AppState {
    fn load_config() -> Config {
        match fs::read_to_string("config.json") {
            Ok(content) => serde_json::from_str(&content).unwrap_or(Config { tickers: vec![] }),
            Err(_) => Config { tickers: vec!["TSLA".to_string(), "2330.TW".to_string()] },
        }
    }

    fn save_config(&self) -> Result<(), Box<dyn Error>> {
        let content = serde_json::to_string_pretty(&self.config)?;
        fs::write("config.json", content)?;
        Ok(())
    }
}

async fn fetch_stock_data(tickers: &[String]) -> String {
    let provider = yahoo::YahooConnector::new().unwrap();
    let mut display_text = String::new();

    for symbol in tickers {
        match provider.get_latest_quotes(symbol, "1d").await {
            Ok(response) => {
                if let Ok(quote) = response.last_quote() {
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
    display_text
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let initial_config = AppState::load_config();
    let mut app_state = AppState {
        stock_data: "Loading...".to_string(),
        input: String::new(),
        input_mode: InputMode::Normal,
        config: initial_config,
    };

    app_state.stock_data = fetch_stock_data(&app_state.config.tickers).await;

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Title
                    Constraint::Min(5),    // Content
                    Constraint::Length(3), // Input box
                ].as_ref())
                .split(f.area());

            let title = Paragraph::new("Rust TUI Investment Dashboard (v0.2)")
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(title, chunks[0]);

            let content = Paragraph::new(app_state.stock_data.as_str())
                .block(Block::default().title("Live Quotes").borders(Borders::ALL));
            f.render_widget(content, chunks[1]);

            let input_title = match app_state.input_mode {
                InputMode::Normal => "Press 'a' to add ticker, 'q' to quit, 'r' to refresh",
                InputMode::Editing => "Enter ticker (e.g. AAPL) and press Enter",
            };
            
            let input_style = match app_state.input_mode {
                InputMode::Normal => Style::default(),
                InputMode::Editing => Style::default().fg(Color::Yellow),
            };

            let input_box = Paragraph::new(app_state.input.as_str())
                .style(input_style)
                .block(Block::default().title(input_title).borders(Borders::ALL));
            f.render_widget(input_box, chunks[2]);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match app_state.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('a') => {
                            app_state.input_mode = InputMode::Editing;
                        },
                        KeyCode::Char('r') => {
                            app_state.stock_data = "Refreshing...".to_string();
                            app_state.stock_data = fetch_stock_data(&app_state.config.tickers).await;
                        },
                        _ => {}
                    },
                    InputMode::Editing => match key.code {
                        KeyCode::Enter => {
                            let new_ticker = app_state.input.drain(..).collect::<String>().to_uppercase();
                            if !new_ticker.is_empty() {
                                app_state.config.tickers.push(new_ticker);
                                let _ = app_state.save_config();
                                app_state.stock_data = "Fetching new data...".to_string();
                                app_state.stock_data = fetch_stock_data(&app_state.config.tickers).await;
                            }
                            app_state.input_mode = InputMode::Normal;
                        },
                        KeyCode::Char(c) => {
                            app_state.input.push(c);
                        },
                        KeyCode::Backspace => {
                            app_state.input.pop();
                        },
                        KeyCode::Esc => {
                            app_state.input_mode = InputMode::Normal;
                            app_state.input.clear();
                        },
                        _ => {}
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
