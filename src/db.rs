use anyhow::Result;
use rusqlite::{params, Connection};
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;
use chrono::{Utc, NaiveDate};

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
        Ok(())
    }

    // --- CRUD 實作 (Phase 1) ---

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

    pub fn add_transaction(
        &self,
        portfolio_id: &str,
        symbol: &str,
        price: Decimal,
        shares: Decimal,
        fee: Decimal,
        t_type: &str,
    ) -> Result<()> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let today = Utc::now().date_naive().to_string();
        
        self.conn.execute(
            "INSERT INTO transactions (id, portfolio_id, symbol, date, price, shares, fee, transaction_type, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                portfolio_id,
                symbol,
                today,
                price.to_string(),
                shares.to_string(),
                fee.to_string(),
                t_type,
                now
            ],
        )?;
        Ok(())
    }
}
