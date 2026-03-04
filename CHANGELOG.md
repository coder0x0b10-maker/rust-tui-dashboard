# Changelog

All notable changes to this project will be documented in this file.

## [1.0.0] - 2026-03-04
### Added
- **Dashboard View**: Real-time summary of portfolio holdings and asset values.
- **Portfolio Management**: Support for multiple portfolios with independent transaction histories.
- **Transaction Logs**: Add, view, and delete buy/sell transactions.
- **Transaction Filtering (F6)**: Search and filter transactions by symbol in the logs view.
- **Rebalance Advice (F5)**: Calculate target allocations vs current holdings and provide buy/sell advice.
- **Config Hot-Reload (F7)**: Automatically detect changes to `config.json` to update the watchlist tickers without restarting the app.
- **API Optimizations (F8)**: Implement local price caching (30s TTL) and circuit breakers to prevent rate-limiting from Yahoo Finance API.
- **Watchlist**: Display tracked tickers from the configuration file directly on the dashboard.
