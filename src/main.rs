mod db;

use std::{
    collections::HashMap,
    error::Error,
    io,
    time::{Duration, Instant},
};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Terminal,
};
use rust_decimal::Decimal;
use time::OffsetDateTime;
use yahoo_finance_api as yahoo;

use db::{Database, Holding, Portfolio, Transaction};

// === Data Structures ===

#[derive(Clone)]
struct StockQuote {
    symbol: String,
    price: f64,
    change: f64,
    change_pct: f64,
    fetched_at: Instant,
}

#[derive(Clone)]
struct PortfolioSummary {
    total_value: f64,
    daily_pnl: f64,
    daily_pnl_pct: f64,
    best_performer: String,
    worst_performer: String,
}

#[derive(Clone)]
struct CircuitBreaker {
    failures: u32,
    last_failure: Option<Instant>,
}

impl CircuitBreaker {
    fn new() -> Self {
        Self {
            failures: 0,
            last_failure: None,
        }
    }

    fn record_failure(&mut self) {
        self.failures += 1;
        self.last_failure = Some(Instant::now());
    }

    fn is_open(&self) -> bool {
        if self.failures >= 3 {
            if let Some(last) = self.last_failure {
                if last.elapsed() > Duration::from_secs(300) {
                    return false;
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

// === View Modes ===

#[derive(Clone, Debug, PartialEq)]
enum ViewMode {
    Dashboard,
    AddTransaction,
    PortfolioSelect,
    TransactionLog,
}

#[derive(Clone, Debug, PartialEq)]
enum TransactionField {
    Symbol,
    Shares,
    Price,
    Fee,
    Type,
}

// === App State ===

struct AppState {
    // Database
    db: Database,
    current_portfolio: Portfolio,
    holdings: Vec<Holding>,
    transactions: Vec<Transaction>,
    all_portfolios: Vec<Portfolio>,
    
    // UI State
    view_mode: ViewMode,
    selected_index: usize,
    status_msg: String,
    error_msg: Option<String>,
    
    // Price Data
    quotes: HashMap<String, StockQuote>,
    cache: HashMap<String, StockQuote>,
    breakers: HashMap<String, CircuitBreaker>,
    last_refresh: Instant,
    
    // Input State
    input_fields: HashMap<String, String>,
    input_field: TransactionField,
    
    // Portfolio
    portfolio_summary: Option<PortfolioSummary>,
}

impl AppState {
    fn new() -> Result<Self> {
        let db = Database::new("investments.db")?;
        let current_portfolio = db.get_or_create_default_portfolio()?;
        let all_portfolios = db.get_all_portfolios()?;
        let holdings = db.calculate_holdings(&current_portfolio.id)?;
        let transactions = db.get_transactions(&current_portfolio.id)?;
        
        let mut input_fields = HashMap::new();
        input_fields.insert("symbol".to_string(), String::new());
        input_fields.insert("shares".to_string(), String::new());
        input_fields.insert("price".to_string(), String::new());
        input_fields.insert("fee".to_string(), "0".to_string());
        input_fields.insert("type".to_string(), "Buy".to_string());
        
        Ok(Self {
            db,
            current_portfolio,
            holdings,
            transactions,
            all_portfolios,
            view_mode: ViewMode::Dashboard,
            selected_index: 0,
            status_msg: " Ready".to_string(),
            error_msg: None,
            quotes: HashMap::new(),
            cache: HashMap::new(),
            breakers: HashMap::new(),
            last_refresh: Instant::now(),
            input_fields,
            input_field: TransactionField::Symbol,
            portfolio_summary: None,
        })
    }

    fn refresh_holdings(&mut self) {
        match self.db.calculate_holdings(&self.current_portfolio.id) {
            Ok(holdings) => {
                self.holdings = holdings;
                self.status_msg = " Holdings refreshed".to_string();
            }
            Err(e) => {
                self.error_msg = Some(format!("Failed to calculate holdings: {}", e));
            }
        }
    }

    fn refresh_transactions(&mut self) {
        match self.db.get_transactions(&self.current_portfolio.id) {
            Ok(transactions) => {
                self.transactions = transactions;
            }
            Err(e) => {
                self.error_msg = Some(format!("Failed to load transactions: {}", e));
            }
        }
    }

    fn switch_portfolio(&mut self, index: usize) {
        if index < self.all_portfolios.len() {
            self.current_portfolio = self.all_portfolios[index].clone();
            self.refresh_holdings();
            self.refresh_transactions();
            self.view_mode = ViewMode::Dashboard;
            self.status_msg = format!(" Switched to: {}", self.current_portfolio.name);
        }
    }

    fn add_transaction(&mut self) -> Result<()> {
        let symbol = self.input_fields.get("symbol").unwrap_or(&String::new()).to_uppercase();
        let shares: Decimal = self.input_fields.get("shares").unwrap_or(&String::new())
            .parse().unwrap_or(Decimal::ZERO);
        let price: Decimal = self.input_fields.get("price").unwrap_or(&String::new())
            .parse().unwrap_or(Decimal::ZERO);
        let fee: Decimal = self.input_fields.get("fee").unwrap_or(&String::new())
            .parse().unwrap_or(Decimal::ZERO);
        let t_type = self.input_fields.get("type").unwrap_or(&"Buy".to_string()).clone();

        if symbol.is_empty() || shares <= Decimal::ZERO || price <= Decimal::ZERO {
            self.error_msg = Some("Invalid input: Please fill all fields correctly".to_string());
            return Ok(());
        }

        let today = chrono::Utc::now().date_naive().to_string();
        
        self.db.add_transaction(
            &self.current_portfolio.id,
            &symbol,
            None,
            &today,
            price,
            shares,
            fee,
            &t_type,
            "TWD",
        )?;

        // Reset input fields
        self.input_fields.insert("symbol".to_string(), String::new());
        self.input_fields.insert("shares".to_string(), String::new());
        self.input_fields.insert("price".to_string(), String::new());
        self.input_fields.insert("fee".to_string(), "0".to_string());
        self.input_fields.insert("type".to_string(), "Buy".to_string());
        
        self.refresh_holdings();
        self.refresh_transactions();
        self.view_mode = ViewMode::Dashboard;
        self.status_msg = format!(" Added {} transaction for {}", t_type, symbol);
        
        Ok(())
    }

    fn delete_selected_transaction(&mut self) {
        if self.selected_index < self.transactions.len() {
            let tx = &self.transactions[self.selected_index];
            if let Err(e) = self.db.delete_transaction(&tx.id) {
                self.error_msg = Some(format!("Failed to delete: {}", e));
            } else {
                self.status_msg = format!(" Deleted transaction: {}", tx.symbol);
                self.refresh_holdings();
                self.refresh_transactions();
            }
        }
    }

    fn delete_selected_portfolio(&mut self) {
        if self.all_portfolios.len() <= 1 {
            self.error_msg = Some("Cannot delete the last portfolio".to_string());
            return;
        }
        
        if self.selected_index < self.all_portfolios.len() {
            let portfolio = &self.all_portfolios[self.selected_index];
            if let Err(e) = self.db.delete_portfolio(&portfolio.id) {
                self.error_msg = Some(format!("Failed to delete: {}", e));
            } else {
                self.status_msg = format!(" Deleted portfolio: {}", portfolio.name);
                self.all_portfolios = self.db.get_all_portfolios().unwrap_or_default();
                self.current_portfolio = self.all_portfolios.first().cloned().unwrap();
                self.refresh_holdings();
                self.refresh_transactions();
                self.view_mode = ViewMode::Dashboard;
            }
        }
    }
}

// === Price Fetching ===

async fn fetch_single_quote(
    provider: &yahoo::YahooConnector,
    symbol: &str,
    now: OffsetDateTime,
    start: OffsetDateTime,
) -> Option<StockQuote> {
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

async fn fetch_all_quotes(
    symbols: &[String],
    cache: &mut HashMap<String, StockQuote>,
    breakers: &mut HashMap<String, CircuitBreaker>,
) -> (HashMap<String, StockQuote>, String) {
    let now = OffsetDateTime::now_utc();
    let start = now - time::Duration::seconds(86400 * 5);
    let cache_ttl = Duration::from_secs(30);

    let mut results = HashMap::new();
    let mut fetched = 0u32;
    let mut cached = 0u32;
    let mut skipped = 0u32;

    for symbol in symbols {
        let breaker = breakers.entry(symbol.clone()).or_insert_with(CircuitBreaker::new);

        if breaker.is_open() {
            skipped += 1;
            if let Some(cached_quote) = cache.get(symbol) {
                results.insert(symbol.clone(), cached_quote.clone());
            }
            continue;
        }

        if let Some(cached_quote) = cache.get(symbol) {
            if cached_quote.fetched_at.elapsed() < cache_ttl {
                cached += 1;
                results.insert(symbol.clone(), cached_quote.clone());
                continue;
            }
        }

        let p = match yahoo::YahooConnector::new() {
            Ok(p) => p,
            Err(_) => continue,
        };

        if let Some(quote) = fetch_single_quote(&p, symbol, now, start).await {
            fetched += 1;
            cache.insert(symbol.clone(), quote.clone());
            breakers.get_mut(symbol).map(|b| b.reset());
            results.insert(symbol.clone(), quote);
        } else {
            breakers.entry(symbol.clone()).or_insert_with(CircuitBreaker::new).record_failure();
            if let Some(cached_quote) = cache.get(symbol) {
                results.insert(symbol.clone(), cached_quote.clone());
            }
        }
    }

    let status = format!(" v0.7 | Fetched: {} | Cached: {} | Skipped: {}", fetched, cached, skipped);
    (results, status)
}

fn calculate_portfolio_summary(holdings: &[Holding], quotes: &HashMap<String, StockQuote>) -> Option<PortfolioSummary> {
    if holdings.is_empty() {
        return None;
    }

    let mut total_value = 0.0;
    let mut daily_pnl = 0.0;
    let mut best: Option<(&str, f64)> = None;
    let mut worst: Option<(&str, f64)> = None;

    for h in holdings {
        let current_price = quotes
            .get(&h.symbol)
            .map(|q| q.price)
            .unwrap_or_else(|| h.avg_cost.to_string().parse().unwrap_or(0.0));
        
        let shares: f64 = h.total_shares.to_string().parse().unwrap_or(0.0);
        let _avg_cost: f64 = h.avg_cost.to_string().parse().unwrap_or(0.0);
        
        total_value += current_price * shares;
        
        if let Some(quote) = quotes.get(&h.symbol) {
            daily_pnl += quote.change * shares;
            
            match &best {
                Some((_, pct)) if quote.change_pct <= *pct => {}
                _ => best = Some((&h.symbol, quote.change_pct)),
            }
            match &worst {
                Some((_, pct)) if quote.change_pct >= *pct => {}
                _ => worst = Some((&h.symbol, quote.change_pct)),
            }
        }
    }

    let prev_value = total_value - daily_pnl;
    let daily_pnl_pct = if prev_value != 0.0 { (daily_pnl / prev_value) * 100.0 } else { 0.0 };

    Some(PortfolioSummary {
        total_value,
        daily_pnl,
        daily_pnl_pct,
        best_performer: best.map(|(s, p)| format!("{} ({:+.2}%)", s, p)).unwrap_or_default(),
        worst_performer: worst.map(|(s, p)| format!("{} ({:+.2}%)", s, p)).unwrap_or_default(),
    })
}

// === Main ===

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app_state = AppState::new()?;

    // Initial price fetch
    let symbols: Vec<String> = app_state.holdings.iter().map(|h| h.symbol.clone()).collect();
    if !symbols.is_empty() {
        let (quotes, status) = fetch_all_quotes(&symbols, &mut app_state.cache, &mut app_state.breakers).await;
        app_state.quotes = quotes;
        app_state.status_msg = status;
    }
    
    app_state.portfolio_summary = calculate_portfolio_summary(&app_state.holdings, &app_state.quotes);

    let auto_refresh_interval = Duration::from_secs(60);

    loop {
        // Auto-refresh prices
        if app_state.last_refresh.elapsed() >= auto_refresh_interval {
            let symbols: Vec<String> = app_state.holdings.iter().map(|h| h.symbol.clone()).collect();
            if !symbols.is_empty() {
                let (quotes, status) = fetch_all_quotes(&symbols, &mut app_state.cache, &mut app_state.breakers).await;
                app_state.quotes = quotes;
                app_state.status_msg = status;
                app_state.portfolio_summary = calculate_portfolio_summary(&app_state.holdings, &app_state.quotes);
            }
            app_state.last_refresh = Instant::now();
        }

        terminal.draw(|f| {
            match app_state.view_mode {
                ViewMode::Dashboard => render_dashboard(&mut app_state, f),
                ViewMode::AddTransaction => render_add_transaction(&mut app_state, f),
                ViewMode::PortfolioSelect => render_portfolio_select(&mut app_state, f),
                ViewMode::TransactionLog => render_transaction_log(&mut app_state, f),
            }
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                handle_key_event(&mut app_state, key.code, key.modifiers).await;
                
                if app_state.view_mode == ViewMode::Dashboard && key.code == KeyCode::Char('q') {
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

fn render_dashboard(app_state: &mut AppState, f: &mut ratatui::Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Length(4),  // Portfolio Summary
            Constraint::Min(10),    // Holdings Table
            Constraint::Length(3),  // Input/Hint
            Constraint::Length(1),  // Status
        ])
        .split(f.area());

    // Title
    let title = Paragraph::new(format!("💰 RUST INVESTMENT DASHBOARD v0.7 [{}]", 
        chrono::Local::now().format("%Y-%m-%d %H:%M")))
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(title, chunks[0]);

    // Portfolio Summary
    let summary_text = match &app_state.portfolio_summary {
        Some(p) => {
            let pnl_sign = if p.daily_pnl >= 0.0 { "+" } else { "" };
            format!(
                " 📂 {} | Total: ${:.2} | Daily P&L: {}${:.2} ({}{:.2}%) | 🏆 {} | 📉 {}",
                app_state.current_portfolio.name,
                p.total_value,
                pnl_sign, p.daily_pnl, pnl_sign, p.daily_pnl_pct,
                p.best_performer, p.worst_performer
            )
        }
        None => format!(" 📂 {} | No holdings yet. Press [a] to add transactions.", 
            app_state.current_portfolio.name),
    };

    let summary = Paragraph::new(summary_text)
        .block(Block::default().title(" Portfolio Summary ").borders(Borders::ALL))
        .style(Style::default().fg(Color::Yellow));
    f.render_widget(summary, chunks[1]);

    // Holdings Table
    let header_cells = ["Symbol", "Shares", "Avg Cost", "Price", "Value", "Return"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows: Vec<Row> = app_state.holdings.iter().map(|h| {
        let price = app_state.quotes
            .get(&h.symbol)
            .map(|q| q.price)
            .unwrap_or_else(|| h.avg_cost.to_string().parse().unwrap_or(0.0));
        
        let shares: f64 = h.total_shares.to_string().parse().unwrap_or(0.0);
        let avg_cost: f64 = h.avg_cost.to_string().parse().unwrap_or(0.0);
        let value = price * shares;
        let return_pct = if avg_cost > 0.0 { ((price - avg_cost) / avg_cost) * 100.0 } else { 0.0 };
        
        let color = if return_pct > 0.0 { Color::Green } else if return_pct < 0.0 { Color::Red } else { Color::White };

        Row::new(vec![
            Cell::from(h.symbol.clone()),
            Cell::from(format!("{:.2}", shares)),
            Cell::from(format!("${:.2}", avg_cost)),
            Cell::from(format!("${:.2}", price)),
            Cell::from(format!("${:.2}", value)),
            Cell::from(format!("{:+.2}%", return_pct)).style(Style::default().fg(color)),
        ])
    }).collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ],
    )
    .header(header)
    .block(Block::default().title(" Holdings ").borders(Borders::ALL));
    f.render_widget(table, chunks[2]);

    // Hint bar
    let hint = Paragraph::new(
        " [P]ortfolio [L]ogs [A]dd [R]efresh [D]elete Holdings [Q]uit"
    )
    .block(Block::default().borders(Borders::ALL))
    .style(Style::default().fg(Color::DarkGray));
    f.render_widget(hint, chunks[3]);

    // Status bar
    let status = if let Some(ref err) = app_state.error_msg {
        format!(" [ERROR] {} ", err)
    } else {
        app_state.status_msg.clone()
    };
    let status_bar = Paragraph::new(status).style(Style::default().fg(Color::DarkGray));
    f.render_widget(status_bar, chunks[4]);
}

fn render_add_transaction(app_state: &mut AppState, f: &mut ratatui::Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(10),
            Constraint::Min(1),
        ])
        .split(f.area());

    // Title
    let title = Paragraph::new("➕ Add Transaction [Esc to Cancel]")
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Yellow));
    f.render_widget(title, chunks[0]);

    // Input fields
    let fields = vec![
        ("Symbol", app_state.input_fields.get("symbol").unwrap_or(&String::new()).clone()),
        ("Shares", app_state.input_fields.get("shares").unwrap_or(&String::new()).clone()),
        ("Price", app_state.input_fields.get("price").unwrap_or(&String::new()).clone()),
        ("Fee", app_state.input_fields.get("fee").unwrap_or(&String::new()).clone()),
        ("Type (Buy/Sell)", app_state.input_fields.get("type").unwrap_or(&String::new()).clone()),
    ];

    let rows: Vec<Row> = fields.iter().enumerate().map(|(i, (label, value))| {
        let is_selected = match app_state.input_field {
            TransactionField::Symbol => i == 0,
            TransactionField::Shares => i == 1,
            TransactionField::Price => i == 2,
            TransactionField::Fee => i == 3,
            TransactionField::Type => i == 4,
        };
        
        let style = if is_selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        Row::new(vec![
            Cell::from(*label).style(style),
            Cell::from(value.clone()).style(style),
        ])
    }).collect();

    let table = Table::new(rows, [Constraint::Percentage(30), Constraint::Percentage(70)])
        .block(Block::default().title(" Input Fields ").borders(Borders::ALL));
    f.render_widget(table, chunks[1]);

    // Hint
    let hint = Paragraph::new(" [Tab] Next Field [Enter] Save [Esc] Cancel ")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(hint, chunks[2]);
}

fn render_portfolio_select(app_state: &mut AppState, f: &mut ratatui::Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Title
    let title = Paragraph::new("📂 Select Portfolio [Esc to Cancel]")
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(title, chunks[0]);

    // Portfolio list
    let rows: Vec<Row> = app_state.all_portfolios.iter().enumerate().map(|(i, p)| {
        let is_selected = i == app_state.selected_index;
        let is_current = p.id == app_state.current_portfolio.id;
        
        let prefix = if is_current { "✓ " } else { "  " };
        let style = if is_selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else if is_current {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::White)
        };

        Row::new(vec![
            Cell::from(format!("{}{}", prefix, p.name)).style(style),
            Cell::from(p.id.split('-').next().unwrap_or(&p.id)).style(style),
        ])
    }).collect();

    let table = Table::new(rows, [Constraint::Percentage(70), Constraint::Percentage(30)])
        .block(Block::default().title(" Portfolios ").borders(Borders::ALL));
    f.render_widget(table, chunks[1]);

    // Hint
    let hint = Paragraph::new(" [↑/↓] Navigate [Enter] Select [D] Delete [N] New [Esc] Cancel ")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(hint, chunks[2]);
}

fn render_transaction_log(app_state: &mut AppState, f: &mut ratatui::Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Title
    let title = Paragraph::new(format!("📜 Transaction History - {} [Esc to Back]", 
        app_state.current_portfolio.name))
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(title, chunks[0]);

    // Transaction list
    let header_cells = ["Date", "Type", "Symbol", "Price", "Shares", "Fee", "Total"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows: Vec<Row> = app_state.transactions.iter().enumerate().map(|(i, tx)| {
        let is_selected = i == app_state.selected_index;
        let style = if is_selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let total: f64 = (tx.price * tx.shares + tx.fee).to_string().parse().unwrap_or(0.0);
        let type_color = match tx.transaction_type.as_str() {
            "Buy" => Color::Green,
            "Sell" => Color::Red,
            _ => Color::Yellow,
        };

        Row::new(vec![
            Cell::from(tx.date.clone()).style(style),
            Cell::from(tx.transaction_type.clone()).style(Style::default().fg(type_color)),
            Cell::from(tx.symbol.clone()).style(style),
            Cell::from(format!("${:.2}", tx.price)).style(style),
            Cell::from(format!("{:.2}", tx.shares)).style(style),
            Cell::from(format!("${:.2}", tx.fee)).style(style),
            Cell::from(format!("${:.2}", total)).style(style),
        ])
    }).collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(15),
            Constraint::Percentage(10),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
        ],
    )
    .header(header)
    .block(Block::default().title(" Transactions ").borders(Borders::ALL));
    f.render_widget(table, chunks[1]);

    // Hint
    let hint = Paragraph::new(" [↑/↓] Navigate [D] Delete [Esc] Back ")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(hint, chunks[2]);
}

async fn handle_key_event(app_state: &mut AppState, code: KeyCode, _modifiers: KeyModifiers) {
    // Clear error on any key press
    app_state.error_msg = None;

    match app_state.view_mode {
        ViewMode::Dashboard => {
            match code {
                KeyCode::Char('p') => {
                    app_state.view_mode = ViewMode::PortfolioSelect;
                    app_state.selected_index = 0;
                }
                KeyCode::Char('a') => {
                    app_state.view_mode = ViewMode::AddTransaction;
                    app_state.input_field = TransactionField::Symbol;
                }
                KeyCode::Char('l') => {
                    app_state.view_mode = ViewMode::TransactionLog;
                    app_state.selected_index = 0;
                }
                KeyCode::Char('r') => {
                    let symbols: Vec<String> = app_state.holdings.iter().map(|h| h.symbol.clone()).collect();
                    if !symbols.is_empty() {
                        app_state.status_msg = " Refreshing prices...".to_string();
                        let (quotes, status) = fetch_all_quotes(&symbols, &mut app_state.cache, &mut app_state.breakers).await;
                        app_state.quotes = quotes;
                        app_state.status_msg = status;
                        app_state.portfolio_summary = calculate_portfolio_summary(&app_state.holdings, &app_state.quotes);
                    }
                    app_state.last_refresh = Instant::now();
                }
                KeyCode::Char('d') => {
                    // Refresh holdings (re-calculate from transactions)
                    app_state.refresh_holdings();
                }
                _ => {}
            }
        }
        ViewMode::AddTransaction => {
            match code {
                KeyCode::Esc => {
                    app_state.view_mode = ViewMode::Dashboard;
                }
                KeyCode::Enter => {
                    let _ = app_state.add_transaction();
                }
                KeyCode::Tab => {
                    app_state.input_field = match app_state.input_field {
                        TransactionField::Symbol => TransactionField::Shares,
                        TransactionField::Shares => TransactionField::Price,
                        TransactionField::Price => TransactionField::Fee,
                        TransactionField::Fee => TransactionField::Type,
                        TransactionField::Type => TransactionField::Symbol,
                    };
                }
                KeyCode::Backspace => {
                    let field_name = match app_state.input_field {
                        TransactionField::Symbol => "symbol",
                        TransactionField::Shares => "shares",
                        TransactionField::Price => "price",
                        TransactionField::Fee => "fee",
                        TransactionField::Type => "type",
                    };
                    if let Some(value) = app_state.input_fields.get_mut(field_name) {
                        value.pop();
                    }
                }
                KeyCode::Char(c) => {
                    let field_name = match app_state.input_field {
                        TransactionField::Symbol => "symbol",
                        TransactionField::Shares => "shares",
                        TransactionField::Price => "price",
                        TransactionField::Fee => "fee",
                        TransactionField::Type => "type",
                    };
                    if let Some(value) = app_state.input_fields.get_mut(field_name) {
                        // Toggle type field
                        if field_name == "type" {
                            *value = if c == 'b' || c == 'B' {
                                "Buy".to_string()
                            } else if c == 's' || c == 'S' {
                                "Sell".to_string()
                            } else {
                                value.clone()
                            };
                        } else {
                            value.push(c);
                        }
                    }
                }
                _ => {}
            }
        }
        ViewMode::PortfolioSelect => {
            match code {
                KeyCode::Esc => {
                    app_state.view_mode = ViewMode::Dashboard;
                }
                KeyCode::Up => {
                    if app_state.selected_index > 0 {
                        app_state.selected_index -= 1;
                    }
                }
                KeyCode::Down => {
                    if app_state.selected_index < app_state.all_portfolios.len().saturating_sub(1) {
                        app_state.selected_index += 1;
                    }
                }
                KeyCode::Enter => {
                    app_state.switch_portfolio(app_state.selected_index);
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    app_state.delete_selected_portfolio();
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    // Create new portfolio
                    let id = app_state.db.add_portfolio(
                        &format!("Portfolio {}", app_state.all_portfolios.len() + 1),
                        r#"{"Stock": 50, "Bond": 30, "Cash": 20}"#
                    );
                    if let Ok(_id) = id {
                        app_state.all_portfolios = app_state.db.get_all_portfolios().unwrap_or_default();
                        app_state.status_msg = " Created new portfolio".to_string();
                    }
                }
                _ => {}
            }
        }
        ViewMode::TransactionLog => {
            match code {
                KeyCode::Esc => {
                    app_state.view_mode = ViewMode::Dashboard;
                }
                KeyCode::Up => {
                    if app_state.selected_index > 0 {
                        app_state.selected_index -= 1;
                    }
                }
                KeyCode::Down => {
                    if app_state.selected_index < app_state.transactions.len().saturating_sub(1) {
                        app_state.selected_index += 1;
                    }
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    app_state.delete_selected_transaction();
                }
                _ => {}
            }
        }
    }
}
