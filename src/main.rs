use std::{io, time::{Duration, Instant}, error::Error, fs, collections::HashMap};
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
    fetched_at: Instant,
}

#[derive(Clone)]
struct CircuitBreaker {
    failures: u32,
    last_failure: Option<Instant>,
}

impl CircuitBreaker {
    fn new() -> Self {
        Self { failures: 0, last_failure: None }
    }

    fn record_failure(&mut self) {
        self.failures += 1;
        self.last_failure = Some(Instant::now());
    }

    fn is_open(&self) -> bool {
        if self.failures >= 3 {
            // Reset after 5 minutes cooldown
            if let Some(last) = self.last_failure {
                if last.elapsed() > Duration::from_secs(300) {
                    return false; // Allow retry after cooldown
                }
            }
            return true;
        }
        false
    }

    fn reset(&mut self) {
        self.failures = 0;
        self.last_failure = None;
    }
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
    cache: HashMap<String, StockQuote>,
    breakers: HashMap<String, CircuitBreaker>,
    last_refresh: Instant,
    status_msg: String,
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

/// v0.5: Fetch a single ticker quote with circuit breaker support
async fn fetch_single_quote(
    provider: &yahoo::YahooConnector,
    symbol: &str,
    now: OffsetDateTime,
    start: OffsetDateTime,
) -> Option<StockQuote> {
    // Try historical data first
    if let Ok(response) = provider.get_quote_history(symbol, start, now).await {
        if let Ok(quotes) = response.quotes() {
            if quotes.len() >= 2 {
                let last = quotes.last().unwrap();
                let prev = quotes[quotes.len() - 2].close;
                let price = last.close;
                let change = price - prev;
                let change_pct = if prev != 0.0 { (change / prev) * 100.0 } else { 0.0 };

                return Some(StockQuote {
                    symbol: symbol.to_string(),
                    price,
                    change,
                    change_pct,
                    fetched_at: Instant::now(),
                });
            }
        }
    }

    // Fallback to latest quote
    if let Ok(response) = provider.get_latest_quotes(symbol, "1d").await {
        if let Ok(quote) = response.last_quote() {
            return Some(StockQuote {
                symbol: symbol.to_string(),
                price: quote.close,
                change: 0.0,
                change_pct: 0.0,
                fetched_at: Instant::now(),
            });
        }
    }

    None
}

/// v0.5: Batch concurrent fetch with caching and circuit breaker
async fn fetch_all_quotes_batch(
    tickers: &[String],
    cache: &mut HashMap<String, StockQuote>,
    breakers: &mut HashMap<String, CircuitBreaker>,
) -> (Vec<StockQuote>, String) {
    let _provider = match yahoo::YahooConnector::new() {
        Ok(p) => p,
        Err(e) => return (Vec::new(), format!("Provider error: {}", e)),
    };

    let now = OffsetDateTime::now_utc();
    let start = now - time::Duration::seconds(86400 * 5);
    let cache_ttl = Duration::from_secs(30);

    let mut results = Vec::new();
    let mut fetched = 0u32;
    let mut cached = 0u32;
    let mut skipped = 0u32;

    // Spawn concurrent fetch tasks for non-cached, non-broken tickers
    let mut handles = Vec::new();
    let mut fetch_order: Vec<(usize, String)> = Vec::new();

    for (i, symbol) in tickers.iter().enumerate() {
        let breaker = breakers.entry(symbol.clone()).or_insert_with(CircuitBreaker::new);

        // Check circuit breaker
        if breaker.is_open() {
            skipped += 1;
            // Use stale cache if available
            if let Some(cached_quote) = cache.get(symbol) {
                results.push(cached_quote.clone());
            }
            continue;
        }

        // Check cache TTL
        if let Some(cached_quote) = cache.get(symbol) {
            if cached_quote.fetched_at.elapsed() < cache_ttl {
                cached += 1;
                results.push(cached_quote.clone());
                continue;
            }
        }

        // Need to fetch
        let sym = symbol.clone();
        let p = yahoo::YahooConnector::new().unwrap();
        let handle = tokio::spawn(async move {
            fetch_single_quote(&p, &sym, now, start).await
        });
        handles.push(handle);
        fetch_order.push((i, symbol.clone()));
    }

    // Await all concurrent fetches
    let fetch_results: Vec<_> = futures::future::join_all(handles).await;
    
    for (idx, (_, symbol)) in fetch_order.iter().enumerate() {
        if let Ok(result) = &fetch_results[idx] {
            if let Some(quote) = result {
                fetched += 1;
                cache.insert(symbol.clone(), quote.clone());
                breakers.get_mut(symbol).map(|b| b.reset());
                results.push(quote.clone());
            } else {
                breakers.entry(symbol.clone())
                    .or_insert_with(CircuitBreaker::new)
                    .record_failure();
                // Use stale cache as fallback
                if let Some(cached_quote) = cache.get(symbol) {
                    results.push(cached_quote.clone());
                }
            }
        }
    }

    // Sort results by original ticker order
    results.sort_by(|a, b| {
        let pos_a = tickers.iter().position(|t| t == &a.symbol).unwrap_or(999);
        let pos_b = tickers.iter().position(|t| t == &b.symbol).unwrap_or(999);
        pos_a.cmp(&pos_b)
    });

    let status = format!(
        " v0.5 | Fetched: {} | Cached: {} | Skipped: {} | Total: {} ",
        fetched, cached, skipped, results.len()
    );

    (results, status)
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
        cache: HashMap::new(),
        breakers: HashMap::new(),
        last_refresh: Instant::now(),
        status_msg: String::from(" Loading..."),
    };

    let (quotes, status) = fetch_all_quotes_batch(
        &app_state.config.tickers,
        &mut app_state.cache,
        &mut app_state.breakers,
    ).await;
    app_state.quotes = quotes;
    app_state.status_msg = status;

    let auto_refresh_interval = Duration::from_secs(60);

    loop {
        // Auto-refresh every 60 seconds
        if app_state.last_refresh.elapsed() >= auto_refresh_interval {
            let (quotes, status) = fetch_all_quotes_batch(
                &app_state.config.tickers,
                &mut app_state.cache,
                &mut app_state.breakers,
            ).await;
            app_state.quotes = quotes;
            app_state.status_msg = status;
            app_state.last_refresh = Instant::now();
        }

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(5),
                    Constraint::Length(3),
                    Constraint::Length(1),
                ])
                .split(f.area());

            let title = Paragraph::new("Rust TUI Investment Dashboard (v0.5)")
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().fg(Color::Cyan));
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
                InputMode::Normal => " [a] Add  [d] Delete  [r] Refresh  [q] Quit ",
                InputMode::Editing => " Enter Ticker and Press Enter (Esc to Cancel) ",
            };

            let input_box = Paragraph::new(app_state.input.as_str())
                .block(Block::default().title(input_hint).borders(Borders::ALL))
                .style(match app_state.input_mode {
                    InputMode::Editing => Style::default().fg(Color::Yellow),
                    _ => Style::default(),
                });
            f.render_widget(input_box, chunks[2]);

            // Status bar
            let status_bar = Paragraph::new(app_state.status_msg.as_str())
                .style(Style::default().fg(Color::DarkGray));
            f.render_widget(status_bar, chunks[3]);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match app_state.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('a') => app_state.input_mode = InputMode::Editing,
                        KeyCode::Char('d') => {
                            // Delete last ticker
                            if !app_state.config.tickers.is_empty() {
                                let removed = app_state.config.tickers.pop().unwrap();
                                app_state.cache.remove(&removed);
                                app_state.breakers.remove(&removed);
                                let _ = app_state.save_config();
                                app_state.status_msg = format!(" Removed: {} ", removed);
                            }
                        }
                        KeyCode::Char('r') => {
                            app_state.status_msg = " Refreshing...".to_string();
                            let (quotes, status) = fetch_all_quotes_batch(
                                &app_state.config.tickers,
                                &mut app_state.cache,
                                &mut app_state.breakers,
                            ).await;
                            app_state.quotes = quotes;
                            app_state.status_msg = status;
                            app_state.last_refresh = Instant::now();
                        }
                        _ => {}
                    },
                    InputMode::Editing => match key.code {
                        KeyCode::Enter => {
                            let new_ticker = app_state.input.drain(..).collect::<String>().to_uppercase();
                            if !new_ticker.is_empty() && !app_state.config.tickers.contains(&new_ticker) {
                                app_state.config.tickers.push(new_ticker);
                                let _ = app_state.save_config();
                                let (quotes, status) = fetch_all_quotes_batch(
                                    &app_state.config.tickers,
                                    &mut app_state.cache,
                                    &mut app_state.breakers,
                                ).await;
                                app_state.quotes = quotes;
                                app_state.status_msg = status;
                                app_state.last_refresh = Instant::now();
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
