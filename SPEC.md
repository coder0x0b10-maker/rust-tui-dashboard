# Rust TUI Investment Dashboard - 完整規格書

> **負責人**: 大哈 (Da-Ha Strategist)
> **狀態**: Final Draft
> **版本**: 1.0
> **建立日期**: 2026-02-24

---

## 1. 專案願景

### 1.1 目標
建立一個 **終端機介面的投資組合管理系統**，讓統帥 (6pence) 能夠：
- 即時追蹤投資組合表現
- 記錄買賣交易
- 獲得再平衡建議
- 在 RPi5 上高效運行

### 1.2 核心價值
- **輕量**: TUI 介面，無 GUI 開銷
- **離線優先**: 本地 SQLite 資料庫
- **即時**: 即時股價更新
- **跨平台**: Linux (RPi5)、macOS、Windows

### 1.3 成功指標
| 指標 | 目標 |
|------|------|
| 啟動時間 | < 3 秒 |
| 記憶體使用 | < 50 MB |
| 股價更新延遲 | < 2 秒 |
| RPi5 可運行 | ✅ |

---

## 2. 功能需求

### 2.1 MVP 功能 (v1.0)

| # | 功能 | 優先級 | 狀態 |
|---|------|--------|------|
| F1 | 即時股價監控 | P0 | ✅ v0.6 已實作 |
| F2 | 投資組合總覽 | P0 | ✅ v0.6 已實作 |
| F3 | 多 Portfolio 管理 | P0 | ❌ 待實作 |
| F4 | 交易記錄 (CRUD) | P0 | ❌ 待實作 |
| F5 | Holdings 計算 | P0 | ❌ 待實作 |
| F6 | 再平衡建議 | P1 | ❌ 待實作 |
| F7 | 目標配置設定 | P1 | ❌ 待實作 |
| F8 | SQLite 持久化 | P0 | ❌ 待實作 |

### 2.2 未來功能 (v2.0+)

| # | 功能 | 優先級 |
|---|------|--------|
| F9 | 多幣別支援 (TWD/USD) | P2 |
| F10 | 績效報表 (月/季/年) | P2 |
| F11 | 股息追蹤 | P3 |
| F12 | CSV 匯出/匯入 | P3 |
| F13 | 股票搜尋 (symbol autocomplete) | P3 |

---

## 3. 技術架構

### 3.1 系統架構圖

```
┌─────────────────────────────────────────────────────────────┐
│                      TUI Layer (Ratatui)                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ Dashboard   │  │ Transaction │  │   Rebalance View    │  │
│  │   View      │  │    View     │  │                     │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ Portfolio   │  │ Transaction │  │   Rebalance         │  │
│  │  Manager    │  │   Manager   │  │   Calculator        │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
│  ┌─────────────┐  ┌─────────────┐                           │
│  │ Return      │  │   Price     │                           │
│  │ Calculator  │  │   Fetcher   │                           │
│  └─────────────┘  └─────────────┘                           │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      Data Layer                             │
│  ┌─────────────┐  ┌─────────────┐                           │
│  │   SQLite    │  │   Yahoo     │                           │
│  │  (rusqlite) │  │  Finance API│                           │
│  └─────────────┘  └─────────────┘                           │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 資料模型

```rust
// === Portfolio ===
struct Portfolio {
    id: Uuid,
    name: String,
    target_allocations: HashMap<AssetClass, f64>,  // 目標配置
    base_currency: Currency,                       // TWD / USD
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

// === Transaction ===
struct Transaction {
    id: Uuid,
    portfolio_id: Uuid,
    symbol: String,              // e.g., "2330.TW", "AAPL"
    name: Option<String>,        // e.g., "台積電", "Apple"
    date: NaiveDate,
    price: Decimal,              // 買入價
    shares: Decimal,             // 股數
    fee: Decimal,                // 手續費
    transaction_type: TransactionType,
    currency: Currency,
}

enum TransactionType {
    Buy,
    Sell,
    Dividend,
}

// === Holding (計算結果) ===
struct Holding {
    symbol: String,
    total_shares: Decimal,       // 總股數
    avg_cost: Decimal,           // 平均成本
    current_price: Decimal,      // 即時價格
    market_value: Decimal,       // 市值
    unrealized_pnl: Decimal,     // 未實現損益
    unrealized_pnl_pct: f64,     // 報酬率 %
}

// === AssetClass ===
enum AssetClass {
    TaiwanStock,    // 台股
    USStock,        // 美股
    ETF,            // ETF
    Bond,           // 債券
    Cash,           // 現金
}

// === Currency ===
enum Currency {
    TWD,
    USD,
}
```

### 3.3 SQLite Schema

```sql
-- portfolios
CREATE TABLE portfolios (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    target_allocations TEXT NOT NULL,  -- JSON
    base_currency TEXT NOT NULL DEFAULT 'TWD',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- transactions
CREATE TABLE transactions (
    id TEXT PRIMARY KEY,
    portfolio_id TEXT NOT NULL,
    symbol TEXT NOT NULL,
    name TEXT,
    date TEXT NOT NULL,
    price TEXT NOT NULL,        -- Decimal as TEXT
    shares TEXT NOT NULL,
    fee TEXT NOT NULL,
    transaction_type TEXT NOT NULL,  -- 'Buy' | 'Sell' | 'Dividend'
    currency TEXT NOT NULL DEFAULT 'TWD',
    created_at TEXT NOT NULL,
    FOREIGN KEY (portfolio_id) REFERENCES portfolios(id)
);

-- Indexes
CREATE INDEX idx_transactions_portfolio ON transactions(portfolio_id);
CREATE INDEX idx_transactions_symbol ON transactions(symbol);
CREATE INDEX idx_transactions_date ON transactions(date);
```

### 3.4 資料流設計

```
┌─────────────────────────────────────────────────────────────────┐
│                        User Input                                │
│  [a] Add Transaction  [r] Rebalance  [p] Switch Portfolio       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      App State (Arc<Mutex>)                     │
│  - current_portfolio: Option<Portfolio>                         │
│  - holdings: Vec<Holding>                                       │
│  - quotes: HashMap<String, StockQuote>                          │
│  - view: ViewMode                                               │
└─────────────────────────────────────────────────────────────────┘
                              │
          ┌───────────────────┼───────────────────┐
          ▼                   ▼                   ▼
┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
│  SQLite Layer    │ │  Price Fetcher   │ │  Calculator      │
│  (async)         │ │  (background)    │ │  (on demand)     │
└──────────────────┘ └──────────────────┘ └──────────────────┘
          │                   │                   │
          ▼                   ▼                   ▼
    [Load/Save]        [Yahoo API]          [Holdings]
    Transactions       [每 60s 更新]        [P&L]
                                              [Rebalance]
```

---

## 4. API 設計

### 4.1 Yahoo Finance API 格式

| 市場 | Symbol 格式 | 範例 |
|------|------------|------|
| 台股 | `{code}.TW` | `2330.TW` (台積電) |
| 美股 | `{symbol}` | `AAPL`, `TSLA` |
| ETF | `{symbol}` | `VOO`, `BND` |

### 4.2 Rate Limiting 策略

- **Circuit Breaker**: 連續失敗 3 次後暫停 5 分鐘
- **Cache TTL**: 30 秒
- **Concurrent Fetch**: 使用 `futures::join_all` 批次查詢
- **Auto Refresh**: 每 60 秒自動更新

---

## 5. 開發里程碑

### Phase 1: Foundation (v0.7) - 2 days
- [ ] SQLite 資料庫初始化
- [ ] Portfolio CRUD
- [ ] Transaction CRUD
- [ ] 基本 Holdings 計算

### Phase 2: Core Features (v0.8) - 3 days
- [ ] Holdings 自動計算 (avg cost)
- [ ] 多 Portfolio 切換
- [ ] 再平衡計算邏輯
- [ ] Rebalance View UI

### Phase 3: Polish (v0.9) - 2 days
- [ ] 錯誤處理優化
- [ ] 鍵盤快捷鍵完善
- [ ] 設定畫面
- [ ] 效能優化

### Phase 4: Release (v1.0) - 1 day
- [ ] 六哈驗證
- [ ] 文件完善
- [ ] Release Notes

---

## 6. 驗收標準 (六哈檢核清單)

### 功能驗收
- [ ] **F1**: 能查詢台股、美股即時價格
- [ ] **F2**: 能顯示投資組合總市值、日報酬
- [ ] **F3**: 能建立、切換多個 Portfolio
- [ ] **F4**: 能新增、修改、刪除交易記錄
- [ ] **F5**: Holdings 自動計算平均成本
- [ ] **F6**: Rebalance 顯示買賣建議
- [ ] **F7**: 能設定目標配置比例
- [ ] **F8**: 資料持久化到 SQLite

### 效能驗收
- [ ] 啟動時間 < 3 秒
- [ ] 記憶體 < 50 MB
- [ ] RPi5 可流暢運行

### 穩定性驗收
- [ ] API 失敗不會 crash
- [ ] 錯誤訊息清楚易懂
- [ ] 資料不會遺失

---

## 7. 風險與對策

| 風險 | 可能性 | 影響 | 對策 |
|------|--------|------|------|
| Yahoo API 限流 | 中 | 高 | Circuit Breaker + Cache |
| 台股格式不正確 | 低 | 中 | 先驗證 `2330.TW` 格式 |
| SQLite migration 複雜 | 低 | 中 | 使用 `rusqlite_migration` crate |
| RPi5 效能不足 | 低 | 低 | 已驗證 v0.6 可運行 |

---

## 8. 附錄

### 8.1 Dependencies (Cargo.toml)

```toml
[package]
name = "rust-tui-dashboard"
version = "1.0.0"
edition = "2021"

[dependencies]
# TUI
ratatui = "0.30"
crossterm = "0.29"

# Database
rusqlite = { version = "0.32", features = ["bundled"] }

# Async
tokio = { version = "1", features = ["full"] }
futures = "0.3"

# HTTP & API
reqwest = { version = "0.12", features = ["json"] }
yahoo_finance_api = "4.1"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Date & Time
chrono = { version = "0.4", features = ["serde"] }
time = { version = "0.3", features = ["formatting", "macros"] }

# Math
rust_decimal = "1.36"
rust_decimal_macros = "1.36"

# Utils
uuid = { version = "1.11", features = ["v4", "serde"] }
anyhow = "1.0"
thiserror = "2.0"
```

### 8.2 鍵盤快捷鍵

| 按鍵 | 功能 |
|------|------|
| `p` | 切換 Portfolio |
| `a` | 新增交易 |
| `e` | 編輯交易 |
| `d` | 刪除交易 |
| `r` | 重新整理股價 |
| `b` | 查看再平衡建議 |
| `s` | 設定 |
| `h` | 幫助 |
| `q` | 離開 |
| `Esc` | 返回/取消 |
| `Enter` | 確認 |
| `↑/↓` | 選擇項目 |
| `Tab` | 切換欄位 |

---

## 9. 審核記錄

| 日期 | 審核者 | 狀態 | 備註 |
|------|--------|------|------|
| 2026-02-24 | 大哈 | ✅ 完成 | 送交三哈設計 UI |
| | 三哈 | ⏳ 待審 | - |
| | 二哈 | ⏳ 待審 | - |
| | 六哈 | ⏳ 待審 | - |

---

**下一步**: 送交 **三哈** 進行 UI 設計完善
