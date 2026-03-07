# Rust TUI Dashboard v1.1 - Investment Radar Extension Specification

> **Assignee**: Da-Ha Strategist
> **Status**: Drafting
> **Version**: 1.1
> **Last Updated**: 2026-03-07

---

## 1. Extension Goal
Builds upon the existing investment dashboard by adding an "Investment Radar" feature, integrating technical indicator analysis and a visual alert system to track market trends and timing.

## 2. Technical Indicators
Compute and display basic technical indicators within the TUI.
- **MACD (Moving Average Convergence Divergence)**: Analyzes trends and momentum.
  - Calculation: Fast line (12-day EMA) - Slow line (26-day EMA), with a 9-day EMA signal line.
  - TUI Display: Shows histogram values or trend symbols (e.g., `▲` / `▼`).
- **RSI (Relative Strength Index)**: Determines overbought or oversold conditions.
  - Calculation: Based on 14-day price variations.
  - TUI Display: Score (0-100) along with status labels (Overbought/Oversold).

## 3. Visual Alert System
Defines color shifts or blinking logic in the TUI interface when specific thresholds are triggered.
- **RSI Alerts**:
  - `RSI < 30` (Oversold): Text becomes **Green** or Bold, indicating a potential buying opportunity.
  - `RSI > 70` (Overbought): Text becomes **Red** or Blinks, warning of overheating risks.
- **MACD Alerts**:
  - Golden Cross (MACD crosses above signal): Row or label highlighted in **Light Green**.
  - Death Cross (MACD crosses below signal): Row or label highlighted in **Light Red**.

## 4. Async Data Flow Optimization
Optimizes existing Yahoo Finance API fetching logic for parallel processing of multiple indicators.
- **Batch & Concurrent Requests**: Use `tokio::spawn` to fetch multiple assets asynchronously while limiting concurrency (e.g., using a `Semaphore`) to avoid rate limits.
- **Caching Mechanism**: Implement a short-lived in-memory cache for historical data to reduce network overhead on recalculations.
- **Background Computation**: Offload MACD and RSI calculations to a background blocking thread pool (`tokio::task::spawn_blocking`) after fetching history to avoid blocking the main TUI rendering thread.

## 5. Extended UI Design
Plans a new "Radar View" tab layout built on top of the original `UI_DESIGN.md`.
- **Radar View Layout**:
  - **Top Navigation Bar**: Append `[3] Radar` tab.
  - **Left Sidebar**: Watchlist showing Tickers, Current Price, and Change %.
  - **Right Indicator Panel**:
    - Top Half: Detailed quote and RSI gauge (using TUI Gauge or Sparks chart).
    - Bottom Half: MACD trend chart and recent signal logs.
  - **Bottom Status Bar**: Displays last data update timestamp and API request status.
