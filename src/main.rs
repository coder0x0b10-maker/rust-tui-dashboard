use std::{io, time::Duration, error::Error, fs};
use ratatui::{
    Terminal, backend::CrosstermBackend,
    widgets::{Block, Borders, Paragraph, Table, Row, Cell},
    layout::{Layout, Constraint, Direction},
    style::{Style, Color, Modifier}
};
use crossterm::{execute, terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}, event::{self, Event, KeyCode}};
use yahoo_finance_api as yahoo;
use serde::{Serialize, Deserialize};
use time::OffsetDateTime;

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
    let now = OffsetDateTime::now_utc();
    let start = now - Duration::from_secs(86400 * 5); // 抓 5 天內數據

    for symbol in tickers {
        if let Ok(response) = provider.get_quote_history(symbol, start, now).await {
            if let Ok(quotes) = response.quotes() {
                if quotes.len() >= 2 {
                    let last = quotes.last().unwrap();
                    let prev = quotes[quotes.len() - 2].close;
                    let price = last.close;
                    let change = price - prev;
                    let change_pct = if prev != 0.0 { (change / prev) * 100.0 } else { 0.0 };

                    results.push(StockQuote {
                        symbol: symbol.clone(),
                        price,
                        change,
                        change_pct,
                    });
                    continue;
                }
            }
        }
        if let Ok(response) = provider.get_latest_quotes(symbol, "1d").await {
            if let Ok(quote) = response.last_quote() {
                results.push(StockQuote {
                    symbol: symbol.clone(),
                    price: quote.close,
                    change: 0.0,
                    change_pct: 0.0,
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
                    Constraint::Length(3),
                    Constraint::Min(5),
                    Constraint::Length(3),
                ])
                .split(f.area());

            let title = Paragraph::new("Rust TUI Investment Dashboard (v0.4.2)")
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(title, chunks[0]);

            let header_cells = ["TICKER", "PRICE", "CHANGE", "% CHANGE"]
                .iter()
                .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan)));
            let header = Row::new(header_cells).height(1).bottom_margin(1);

            let rows = app_state.quotes.iter().map(|q| {
                let color = if q.change > 0.0 {
                    Color::Red
                } else if q.change < 0.0 {
                    Color::Green
                } else {
                    Color::White
                };

                let cells = vec![
                    Cell::from(q.symbol.clone()),
                    Cell::from(format!("{:.2}", q.price)),
                    Cell::from(format!("{:.2}", q.change)).style(Style::default().fg(color)),
                    Cell::from(format!("{:.2}%", q.change_pct)).style(Style::default().fg(color)),
                ];
                Row::new(cells)
            });

            let table = Table::new(rows, [
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .header(header)
            .block(Block::default().title(" Market Watch ").borders(Borders::ALL));

            f.render_widget(table, chunks[1]);

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
