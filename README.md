# Rust TUI Dashboard

A terminal-based investment dashboard application built with Rust and `ratatui`. It tracks portfolios, manages transactions, pulls live market data via Yahoo Finance API, and calculates rebalancing needs.

## Features (F1-F8)
- **F1: Dashboard View** - Real-time tracking of holdings, daily P&L, best/worst performers.
- **F2: Transaction Management** - Create, view, and delete buy/sell transactions via a TUI interface.
- **F3: Multi-Portfolio Support** - Switch between multiple independent portfolios.
- **F4: Rebalancing Engine** - Compares current allocations with targets and provides actionable buy/sell advice.
- **F5: Watchlist Tracking** - Tracks specific symbols alongside your portfolio holdings.
- **F6: Search & Filtering** - Search and filter your transaction history by ticker symbol.
- **F7: Config Hot-Reload** - Automatically detects changes to `config.json` and updates the watchlist without restarting.
- **F8: Performance & API Optimizations** - Incorporates local price caching (30s TTL) and a Circuit Breaker pattern to prevent rate limits from Yahoo Finance.

## Installation & Setup

1. Ensure you have the Rust toolchain installed:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. Clone the repository and navigate to the directory:
   ```bash
   git clone <repo_url> rust-tui-dashboard
   cd rust-tui-dashboard
   ```

3. Create a `config.json` for your watchlist:
   ```json
   {
     "tickers": ["AAPL", "TSLA", "MSFT"]
   }
   ```

4. Run the application:
   ```bash
   cargo run --release
   ```

## Controls
- `[P]` Select Portfolio
- `[L]` Transaction Logs
- `[F]` Filter Transactions
- `[A]` Add Transaction
- `[B]` Calculate Rebalance
- `[R]` Refresh Market Data
- `[Q]` Quit
- `[Esc]` Back / Cancel
