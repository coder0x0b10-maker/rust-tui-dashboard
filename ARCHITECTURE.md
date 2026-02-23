# Rust TUI Investment Dashboard - 架構設計

> **負責人**: 大哈 (Da-Ha Strategist)  
> **狀態**: Drafting  
> **最後更新**: 2026-02-24

---

## 1. 資料模型設計

### Portfolio (投資組合)
```rust
struct Portfolio {
    id: Uuid,
    name: String,
    target_allocations: HashMap<AssetClass, f64>, // 目標配置比例
    created_at: DateTime,
}
```

### Transaction (交易記錄)
```rust
struct Transaction {
    id: Uuid,
    portfolio_id: Uuid,
    symbol: String,           // 股票代碼
    name: Option<String>,     // 股票名稱
    date: NaiveDate,          // 交易日期
    price: Decimal,           // 買入價
    shares: Decimal,          // 股數
    fee: Decimal,             // 手續費
    transaction_type: TransactionType, // Buy / Sell
}
```

### Holding (目前持股)
```rust
struct Holding {
    symbol: String,
    total_shares: Decimal,    // 總股數
    avg_cost: Decimal,        // 平均成本
    current_price: Decimal,   // 目前價格 (from API)
    market_value: Decimal,    // 目前市值
}
```

### AssetClass (資產類別)
```rust
enum AssetClass {
    TaiwanStock,   // 台股
    USStock,       // 美股
    Bond,          // 債券
    Cash,          // 現金
    ETF,           // ETF
    Crypto,        // 加密貨幣 (未來)
}
```

---

## 2. 功能模組架構

```
┌─────────────────────────────────────────────────┐
│                  TUI Layer (Ratatui)             │
│  ┌──────────┐ ┌──────────┐ ┌──────────────────┐ │
│  │Portfolio │ │ Holdings │ │ Rebalance View   │ │
│  │  View    │ │  View    │ │                  │ │
│  └──────────┘ └──────────┘ └──────────────────┘ │
└─────────────────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────┐
│              Business Logic Layer                │
│  ┌──────────────┐ ┌──────────────────────────┐  │
│  │Portfolio Mgr │ │  Rebalance Calculator    │  │
│  └──────────────┘ └──────────────────────────┘  │
│  ┌──────────────┐ ┌──────────────────────────┐  │
│  │Return Calc   │ │  Price Fetcher           │  │
│  └──────────────┘ └──────────────────────────┘  │
└─────────────────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────┐
│               Data Layer                         │
│  ┌──────────────┐ ┌──────────────────────────┐  │
│  │   SQLite     │ │  Price API Client        │  │
│  │  (rusqlite)  │ │  (reqwest + serde)       │  │
│  └──────────────┘ └──────────────────────────┘  │
└─────────────────────────────────────────────────┘
```

---

## 3. 技術選項評估

### 資料儲存
| 選項 | 優點 | 缺點 | 建議 |
|------|------|------|------|
| **SQLite** | 結構化、查詢快、支援複雜查詢 | 需要 migration | ✅ 推薦 |
| JSON | 簡單、可讀 | 查詢慢、無 schema | ❌ 不推薦 |

### 股價 API
| 選項 | 支援市場 | 免費額度 | 建議 |
|------|----------|----------|------|
| Yahoo Finance API | 台股 + 美股 | 無限制 (非官方) | ✅ 推薦 |
| 證交所 API | 台股 | 無限制 | ✅ 台股備選 |
| Alpha Vantage | 美股 | 5 calls/min | ⚠️ 限制多 |

### TUI Framework
| 選項 | 狀態 | 建議 |
|------|------|------|
| **Ratatui** | 活躍維護、文件完整 | ✅ 確定使用 |

---

## 4. Rust Crates 清單

### 核心
```toml
[dependencies]
ratatui = "0.29"           # TUI framework
crossterm = "0.28"         # Terminal backend
rusqlite = "0.32"          # SQLite
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
chrono = "0.4"             # 日期處理
rust_decimal = "1.36"      # 精確小數計算
uuid = { version = "1.11", features = ["v4"] }
anyhow = "1.0"             # 錯誤處理
```

### 選用
```toml
[dependencies]
tui-input = "0.11"         # 輸入框元件
open = "5.3"               # 開啟瀏覽器 (看圖表)
```

---

## 5. 待研究事項

- [ ] Yahoo Finance API 驗證（台股格式：`2330.TW`）
- [ ] Ratatui 輸入框實作方式
- [ ] SQLite migration 策略
- [ ] 多幣別支援（台幣/美元）

---

## 6. 介面設計（三哈負責）

> 見 `UI_DESIGN.md`（待三哈完成）

