use std::{io, time::Duration, error::Error, fs};
use ratatui::{Terminal, backend::CrosstermBackend, widgets::{Block, Borders, Paragraph}, layout::{Layout, Constraint, Direction}, style::{Style, Color}};
use crossterm::{execute, terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}, event::{self, Event, KeyCode}};
use yahoo_finance_api as yahoo;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone)]
struct Config {
    tickers: Vec<String>,
}

#[derive(Clone)]
struct StockQuote {
    symbol: String,
    price: f64,
    change: f64,
    change_pct: f64,
}

enum InputMode {
    Normal,
    Editing,
}

struct AppState {
    quotes: Vec<StockQuote>,
    input: String,
    input_mode: InputMode,
    config: Config,
}

impl AppState {
    fn load_config() -> Config {
        match fs::read_to_string("config.json") {
            Ok(content) => serde_json::from_str(&content).unwrap_or(Config { tickers: vec![] }),
            Err(_) => Config { tickers: vec!["TSLA".to_string(), "AAPL".to_string()] },
        }
    }

    fn save_config(&self) -> Result<(), Box<dyn Error>> {
        let content = serde_json::to_string_pretty(&self.config)?;
        fs::write("config.json", content)?;
        Ok(())
    }
}

async fn fetch_all_quotes(tickers: &[String]) -> Vec<StockQuote> {
    let provider = yahoo::YahooConnector::new().unwrap();
    let mut results = Vec::new();

    for symbol in tickers {
        if let Ok(response) = provider.get_latest_quotes(symbol, "1d").await {
            if let Ok(quote) = response.last_quote() {
                // 簡單計算漲跌 (目前的價格 vs 前一根 K 線的收盤價，這裡簡化處理)
                // 正確做法應抓取 interval="1d" 的前一日收盤，此處先實作結構
                let price = quote.close;
                let open = quote.open;
                let change = price - open;
                let change_pct = if open != 0.0 { (change / open) * 100.0 } else { 0.0 };

                results.push(StockQuote {
                    symbol: symbol.clone(),
                    price,
                    change,
                    change_pct,
                });
            }
        }
    }
    results
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
        quotes: Vec::new(),
        input: String::new(),
        input_mode: InputMode::Normal,
        config: initial_config,
    };

    app_state.quotes = fetch_all_quotes(&app_state.config.tickers).await;

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Title
                    Constraint::Min(5),    // Content
                    Constraint::Length(3), // Input
                ].as_ref())
                .split(f.area());

            let title = Paragraph::new("Rust TUI Investment Dashboard (v0.3)")
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(title, chunks[0]);

            // 建立多欄對齊的文字
            let mut display_text = format!("{:<12} {:>12} {:>12} {:>12}\n", "TICKER", "PRICE", "CHANGE", "% CHANGE");
            display_text.push_str(&"-".repeat(52));
            display_text.push('\n');

            for q in &app_state.quotes {
                display_text.push_str(&format!(
                    "{:<12} {:>12.2} {:>12.2} {:>11.2}%\n",
                    q.symbol, q.price, q.change, q.change_pct
                ));
            }

            let content = Paragraph::new(display_text)
                .block(Block::default().title("Market Watch").borders(Borders::ALL));
            f.render_widget(content, chunks[1]);

            let input_hint = match app_state.input_mode {
                InputMode::Normal => " [a] Add  [r] Refresh  [q] Quit ",
                InputMode::Editing => " Enter Ticker and Press Enter (Esc to Cancel) ",
            };

            let input_box = Paragraph::new(app_state.input.as_str())
                .block(Block::default().title(input_hint).borders(Borders::ALL))
                .style(match app_state.input_mode {
                    InputMode::Editing => Style::default().fg(Color::Yellow),
                    _ => Style::default(),
                });
            f.render_widget(input_box, chunks[2]);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match app_state.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('a') => app_state.input_mode = InputMode::Editing,
                        KeyCode::Char('r') => {
                            app_state.quotes = fetch_all_quotes(&app_state.config.tickers).await;
                        }
                        _ => {}
                    },
                    InputMode::Editing => match key.code {
                        KeyCode::Enter => {
                            let new_ticker = app_state.input.drain(..).collect::<String>().to_uppercase();
                            if !new_ticker.is_empty() && !app_state.config.tickers.contains(&new_ticker) {
                                app_state.config.tickers.push(new_ticker);
                                let _ = app_state.save_config();
                                app_state.quotes = fetch_all_quotes(&app_state.config.tickers).await;
                            }
                            app_state.input_mode = InputMode::Normal;
                        }
                        KeyCode::Esc => {
                            app_state.input.clear();
                            app_state.input_mode = InputMode::Normal;
                        }
                        KeyCode::Char(c) => app_state.input.push(c),
                        KeyCode::Backspace => { app_state.input.pop(); }
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
