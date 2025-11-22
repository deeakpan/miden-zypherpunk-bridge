use rusqlite::{Connection, Result as SqlResult};
use std::path::PathBuf;

pub struct DepositTracker {
    conn: Connection,
}

#[derive(Debug)]
pub struct DepositRecord {
    pub recipient_hash: String,
    pub txid: String,
    pub amount: u64,
    pub claimed_at: i64,
}

impl DepositTracker {
    pub fn new(db_path: PathBuf) -> SqlResult<Self> {
        let conn = Connection::open(db_path)?;
        
        // Create deposits table if it doesn't exist
        // NOTE: We only store recipient_hash for privacy - we don't store account_id
        // This prevents double-spending while maintaining privacy
        conn.execute(
            "CREATE TABLE IF NOT EXISTS deposits (
                recipient_hash TEXT PRIMARY KEY,
                txid TEXT NOT NULL,
                amount INTEGER NOT NULL,
                claimed_at INTEGER NOT NULL
            )",
            [],
        )?;
        
        // Create index for faster lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_deposits_txid ON deposits(txid)",
            [],
        )?;
        
        Ok(Self { conn })
    }

    /// Check if a recipient hash has already been claimed
    pub fn is_claimed(&self, recipient_hash: &str) -> SqlResult<bool> {
        let mut stmt = self.conn.prepare(
            "SELECT 1 FROM deposits WHERE recipient_hash = ?1 LIMIT 1"
        )?;
        
        let exists = stmt.exists([recipient_hash])?;
        Ok(exists)
    }

    /// Record a claimed deposit
    /// 
    /// NOTE: We only store recipient_hash, NOT account_id, for privacy.
    /// The bridge doesn't need to know which account claimed the deposit.
    pub fn record_claim(
        &self,
        recipient_hash: &str,
        txid: &str,
        amount: u64,
    ) -> SqlResult<()> {
        let claimed_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        self.conn.execute(
            "INSERT INTO deposits (recipient_hash, txid, amount, claimed_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(recipient_hash) DO NOTHING",
            rusqlite::params![recipient_hash, txid, amount, claimed_at],
        )?;
        
        Ok(())
    }

    /// Get deposit record by recipient hash
    pub fn get_deposit(&self, recipient_hash: &str) -> SqlResult<Option<DepositRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT recipient_hash, txid, amount, claimed_at
             FROM deposits WHERE recipient_hash = ?1"
        )?;
        
        let mut rows = stmt.query_map([recipient_hash], |row| {
            Ok(DepositRecord {
                recipient_hash: row.get(0)?,
                txid: row.get(1)?,
                amount: row.get(2)?,
                claimed_at: row.get(3)?,
            })
        })?;
        
        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }
}

