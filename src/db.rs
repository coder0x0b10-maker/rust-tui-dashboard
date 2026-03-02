use anyhow::Result;
use rusqlite::{params, Connection};
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;
use chrono::Utc;
use serde::{Deserialize, Serialize};

// === Data Models ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    pub id: String,
    pub name: String,
    pub target_allocations: String, // JSON string
    pub base_currency: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub portfolio_id: String,
    pub symbol: String,
    pub name: Option<String>,
    pub date: String,
    pub price: Decimal,
    pub shares: Decimal,
    pub fee: Decimal,
    pub transaction_type: String, // "Buy" | "Sell" | "Dividend"
    pub currency: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct Holding {
    pub symbol: String,
    pub name: Option<String>,
    pub total_shares: Decimal,
    pub avg_cost: Decimal,
    pub total_cost: Decimal,
    pub realized_gain: Decimal,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Database { conn };
        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> Result<()> {
        self.conn.execute("PRAGMA foreign_keys = ON;", [])?;
        
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS portfolios (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                target_allocations TEXT NOT NULL,
                base_currency TEXT NOT NULL DEFAULT 'TWD',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS transactions (
                id TEXT PRIMARY KEY,
                portfolio_id TEXT NOT NULL,
                symbol TEXT NOT NULL,
                name TEXT,
                date TEXT NOT NULL,
                price TEXT NOT NULL,
                shares TEXT NOT NULL,
                fee TEXT NOT NULL,
                transaction_type TEXT NOT NULL,
                currency TEXT NOT NULL DEFAULT 'TWD',
                created_at TEXT NOT NULL,
                FOREIGN KEY (portfolio_id) REFERENCES portfolios(id)
            )",
            [],
        )?;

        // Create indexes
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_transactions_portfolio ON transactions(portfolio_id)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_transactions_symbol ON transactions(symbol)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_transactions_date ON transactions(date)",
            [],
        )?;

        Ok(())
    }

    // === Portfolio CRUD ===

    pub fn add_portfolio(&self, name: &str, target_alloc: &str) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        
        self.conn.execute(
            "INSERT INTO portfolios (id, name, target_allocations, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, name, target_alloc, now, now],
        )?;
        
        Ok(id)
    }

    pub fn get_all_portfolios(&self) -> Result<Vec<Portfolio>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, target_allocations, base_currency, created_at, updated_at 
             FROM portfolios ORDER BY created_at"
        )?;

        let portfolios = stmt.query_map([], |row| {
            Ok(Portfolio {
                id: row.get(0)?,
                name: row.get(1)?,
                target_allocations: row.get(2)?,
                base_currency: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?
        .filter_map(|p| p.ok())
        .collect();

        Ok(portfolios)
    }

    pub fn get_portfolio_by_id(&self, id: &str) -> Result<Option<Portfolio>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, target_allocations, base_currency, created_at, updated_at 
             FROM portfolios WHERE id = ?1"
        )?;

        let mut portfolios = stmt.query_map(params![id], |row| {
            Ok(Portfolio {
                id: row.get(0)?,
                name: row.get(1)?,
                target_allocations: row.get(2)?,
                base_currency: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?
        .filter_map(|p| p.ok());

        Ok(portfolios.next())
    }

    pub fn update_portfolio(&self, id: &str, name: &str, target_alloc: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        
        self.conn.execute(
            "UPDATE portfolios SET name = ?1, target_allocations = ?2, updated_at = ?3 
             WHERE id = ?4",
            params![name, target_alloc, now, id],
        )?;
        
        Ok(())
    }

    pub fn delete_portfolio(&self, id: &str) -> Result<()> {
        // Delete associated transactions first (cascade)
        self.conn.execute("DELETE FROM transactions WHERE portfolio_id = ?1", params![id])?;
        self.conn.execute("DELETE FROM portfolios WHERE id = ?1", params![id])?;
        Ok(())
    }

    // === Transaction CRUD ===

    pub fn add_transaction(
        &self,
        portfolio_id: &str,
        symbol: &str,
        name: Option<&str>,
        date: &str,
        price: Decimal,
        shares: Decimal,
        fee: Decimal,
        t_type: &str,
        currency: &str,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        
        self.conn.execute(
            "INSERT INTO transactions 
             (id, portfolio_id, symbol, name, date, price, shares, fee, transaction_type, currency, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                id,
                portfolio_id,
                symbol,
                name,
                date,
                price.to_string(),
                shares.to_string(),
                fee.to_string(),
                t_type,
                currency,
                now
            ],
        )?;
        
        Ok(id)
    }

    pub fn get_transactions(&self, portfolio_id: &str) -> Result<Vec<Transaction>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, portfolio_id, symbol, name, date, price, shares, fee, transaction_type, currency, created_at 
             FROM transactions 
             WHERE portfolio_id = ?1 
             ORDER BY date DESC"
        )?;

        let transactions = stmt.query_map(params![portfolio_id], |row| {
            Ok(Transaction {
                id: row.get(0)?,
                portfolio_id: row.get(1)?,
                symbol: row.get(2)?,
                name: row.get(3)?,
                date: row.get(4)?,
                price: Decimal::from_str(&row.get::<_, String>(5)?).unwrap_or(Decimal::ZERO),
                shares: Decimal::from_str(&row.get::<_, String>(6)?).unwrap_or(Decimal::ZERO),
                fee: Decimal::from_str(&row.get::<_, String>(7)?).unwrap_or(Decimal::ZERO),
                transaction_type: row.get(8)?,
                currency: row.get(9)?,
                created_at: row.get(10)?,
            })
        })?
        .filter_map(|t| t.ok())
        .collect();

        Ok(transactions)
    }

    pub fn get_transaction_by_id(&self, id: &str) -> Result<Option<Transaction>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, portfolio_id, symbol, name, date, price, shares, fee, transaction_type, currency, created_at 
             FROM transactions WHERE id = ?1"
        )?;

        let mut transactions = stmt.query_map(params![id], |row| {
            Ok(Transaction {
                id: row.get(0)?,
                portfolio_id: row.get(1)?,
                symbol: row.get(2)?,
                name: row.get(3)?,
                date: row.get(4)?,
                price: Decimal::from_str(&row.get::<_, String>(5)?).unwrap_or(Decimal::ZERO),
                shares: Decimal::from_str(&row.get::<_, String>(6)?).unwrap_or(Decimal::ZERO),
                fee: Decimal::from_str(&row.get::<_, String>(7)?).unwrap_or(Decimal::ZERO),
                transaction_type: row.get(8)?,
                currency: row.get(9)?,
                created_at: row.get(10)?,
            })
        })?
        .filter_map(|t| t.ok());

        Ok(transactions.next())
    }

    pub fn update_transaction(
        &self,
        id: &str,
        symbol: &str,
        name: Option<&str>,
        date: &str,
        price: Decimal,
        shares: Decimal,
        fee: Decimal,
        t_type: &str,
        currency: &str,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE transactions 
             SET symbol = ?1, name = ?2, date = ?3, price = ?4, shares = ?5, fee = ?6, 
                 transaction_type = ?7, currency = ?8
             WHERE id = ?9",
            params![
                symbol,
                name,
                date,
                price.to_string(),
                shares.to_string(),
                fee.to_string(),
                t_type,
                currency,
                id
            ],
        )?;
        
        Ok(())
    }

    pub fn delete_transaction(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM transactions WHERE id = ?1", params![id])?;
        Ok(())
    }

    // === Holdings Calculation ===

    pub fn calculate_holdings(&self, portfolio_id: &str) -> Result<Vec<Holding>> {
        let mut transactions = self.get_transactions(portfolio_id)?;
        // Sort by date ascending for sequential calculation
        transactions.sort_by(|a, b| a.date.cmp(&b.date));
        
        // Group by symbol and calculate holdings
        let mut holdings_map: std::collections::HashMap<String, Holding> = std::collections::HashMap::new();
        
        for tx in transactions {
            let entry = holdings_map.entry(tx.symbol.clone()).or_insert(Holding {
                symbol: tx.symbol.clone(),
                name: tx.name.clone(),
                total_shares: Decimal::ZERO,
                avg_cost: Decimal::ZERO,
                total_cost: Decimal::ZERO,
                realized_gain: Decimal::ZERO,
            });

            match tx.transaction_type.as_str() {
                "Buy" => {
                    // Add to total cost and shares
                    let cost = tx.price * tx.shares + tx.fee;
                    entry.total_cost += cost;
                    entry.total_shares += tx.shares;
                    // Recalculate average cost
                    if entry.total_shares > Decimal::ZERO {
                        entry.avg_cost = entry.total_cost / entry.total_shares;
                    }
                }
                "Sell" => {
                    // Calculate realized gain
                    let sell_proceeds = tx.price * tx.shares - tx.fee;
                    let cost_basis = entry.avg_cost * tx.shares;
                    entry.realized_gain += sell_proceeds - cost_basis;
                    
                    // Reduce total cost proportionally
                    entry.total_cost -= cost_basis;
                    entry.total_shares -= tx.shares;
                    
                    // Do NOT update average cost on Sell, it remains the same
                    if entry.total_shares <= Decimal::ZERO {
                        entry.avg_cost = Decimal::ZERO;
                        entry.total_cost = Decimal::ZERO;
                    }
                }
                "Dividend" => {
                    // Dividend adds to realized gain
                    entry.realized_gain += tx.price * tx.shares - tx.fee;
                }
                _ => {}
            }
        }

        // Filter out zero holdings and sort by symbol
        let mut holdings: Vec<Holding> = holdings_map
            .into_values()
            .filter(|h| h.total_shares > Decimal::ZERO)
            .collect();
        
        holdings.sort_by(|a, b| a.symbol.cmp(&b.symbol));
        
        Ok(holdings)
    }

    // === Utility Methods ===

    pub fn get_or_create_default_portfolio(&self) -> Result<Portfolio> {
        let portfolios = self.get_all_portfolios()?;
        
        if let Some(portfolio) = portfolios.first() {
            return Ok(portfolio.clone());
        }
        
        // Create default portfolio
        let id = self.add_portfolio(
            "Default Portfolio",
            r#"{"Stock": 60, "Bond": 30, "Cash": 10}"#
        )?;
        
        Ok(self.get_portfolio_by_id(&id)?.unwrap())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use tempfile::NamedTempFile;

    fn setup_test_db() -> (Database, NamedTempFile) {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();
        (Database::new(path).unwrap(), temp_file)
    }

    #[test]
    fn test_portfolio_crud() {
        let (db, _file) = setup_test_db();
        
        // Add
        let id = db.add_portfolio("Test Portfolio", "{}").unwrap();
        
        // Get
        let p = db.get_portfolio_by_id(&id).unwrap().expect("Portfolio should exist");
        assert_eq!(p.name, "Test Portfolio");
        
        // List
        let all = db.get_all_portfolios().unwrap();
        assert_eq!(all.len(), 1);
        
        // Update
        db.update_portfolio(&id, "Updated Name", "{\"stock\": 100}").unwrap();
        let p_updated = db.get_portfolio_by_id(&id).unwrap().unwrap();
        assert_eq!(p_updated.name, "Updated Name");
        assert_eq!(p_updated.target_allocations, "{\"stock\": 100}");
        
        // Delete
        db.delete_portfolio(&id).unwrap();
        let p_deleted = db.get_portfolio_by_id(&id).unwrap();
        assert!(p_deleted.is_none());
    }

    #[test]
    fn test_transaction_crud() {
        let (db, _file) = setup_test_db();
        let p_id = db.add_portfolio("Test", "{}").unwrap();
        
        // Add
        let tx_id = db.add_transaction(
            &p_id, "AAPL", Some("Apple"), "2023-01-01", 
            dec!(150.0), dec!(10.0), dec!(1.0), "Buy", "USD"
        ).unwrap();
        
        // Get
        let tx = db.get_transaction_by_id(&tx_id).unwrap().expect("Transaction should exist");
        assert_eq!(tx.symbol, "AAPL");
        assert_eq!(tx.price, dec!(150.0));
        
        // List
        let all = db.get_transactions(&p_id).unwrap();
        assert_eq!(all.len(), 1);
        
        // Update
        db.update_transaction(
            &tx_id, "AAPL", Some("Apple Inc"), "2023-01-02", 
            dec!(155.0), dec!(10.0), dec!(2.0), "Buy", "USD"
        ).unwrap();
        let tx_updated = db.get_transaction_by_id(&tx_id).unwrap().unwrap();
        assert_eq!(tx_updated.price, dec!(155.0));
        assert_eq!(tx_updated.fee, dec!(2.0));
        assert_eq!(tx_updated.date, "2023-01-02");
        
        // Delete
        db.delete_transaction(&tx_id).unwrap();
        let tx_deleted = db.get_transaction_by_id(&tx_id).unwrap();
        assert!(tx_deleted.is_none());
    }

    #[test]
    fn test_holdings_calculation() {
        let (db, _file) = setup_test_db();
        let p_id = db.add_portfolio("Test", "{}").unwrap();
        
        // Buy 10 AAPL at $100, fee $10
        db.add_transaction(
            &p_id, "AAPL", None, "2023-01-01", 
            dec!(100.0), dec!(10.0), dec!(10.0), "Buy", "USD"
        ).unwrap();
        
        // Holdings should be 10 shares, avg cost $101.0
        let holdings = db.calculate_holdings(&p_id).unwrap();
        assert_eq!(holdings.len(), 1);
        assert_eq!(holdings[0].symbol, "AAPL");
        assert_eq!(holdings[0].total_shares, dec!(10.0));
        assert_eq!(holdings[0].avg_cost, dec!(101.0)); // (1000 + 10) / 10
        
        // Buy 10 more AAPL at $120, fee $10
        db.add_transaction(
            &p_id, "AAPL", None, "2023-01-02", 
            dec!(120.0), dec!(10.0), dec!(10.0), "Buy", "USD"
        ).unwrap();
        
        // Holdings: 20 shares, total cost 1010 + 1210 = 2220, avg cost $111.0
        let holdings = db.calculate_holdings(&p_id).unwrap();
        assert_eq!(holdings[0].total_shares, dec!(20.0));
        assert_eq!(holdings[0].avg_cost, dec!(111.0));
        
        // Sell 5 AAPL at $150, fee $5
        db.add_transaction(
            &p_id, "AAPL", None, "2023-01-03", 
            dec!(150.0), dec!(5.0), dec!(5.0), "Sell", "USD"
        ).unwrap();
        
        // Realized gain: (150*5 - 5) - (111*5) = 745 - 555 = 190
        // Remaining: 15 shares, total cost 2220 - 555 = 1665, avg cost $111.0
        let holdings = db.calculate_holdings(&p_id).unwrap();
        assert_eq!(holdings[0].total_shares, dec!(15.0));
        assert_eq!(holdings[0].realized_gain, dec!(190.0));
        assert_eq!(holdings[0].avg_cost, dec!(111.0));
    }
}
