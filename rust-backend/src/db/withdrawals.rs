use rusqlite::{Connection, Result as SqlResult};
use std::path::PathBuf;

pub struct WithdrawalTracker {
    conn: Connection,
}

#[derive(Debug)]
pub struct WithdrawalRecord {
    pub commitment: String, // hash(secret) - stored as hex
    pub note_id: String,
    pub amount: u64,
    pub block_number: u32,
    pub created_at: i64,
    pub claimed_at: Option<i64>,
    pub zcash_txid: Option<String>,
}

impl WithdrawalTracker {
    pub fn new(db_path: PathBuf) -> SqlResult<Self> {
        let conn = Connection::open(db_path)?;
        
        // Create withdrawals table if it doesn't exist
        // commitment = hash(secret) - this is what's stored on-chain
        // secret is never stored - user provides it when claiming
        conn.execute(
            "CREATE TABLE IF NOT EXISTS withdrawals (
                commitment TEXT PRIMARY KEY,
                note_id TEXT UNIQUE NOT NULL,
                amount INTEGER NOT NULL,
                block_number INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                claimed_at INTEGER,
                zcash_txid TEXT
            )",
            [],
        )?;
        
        // Create index for faster lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_withdrawals_note_id ON withdrawals(note_id)",
            [],
        )?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_withdrawals_claimed ON withdrawals(claimed_at)",
            [],
        )?;
        
        Ok(Self { conn })
    }

    /// Record a new withdrawal commitment
    pub fn record_withdrawal(
        &self,
        commitment: &str,
        note_id: &str,
        amount: u64,
        block_number: u32,
    ) -> SqlResult<()> {
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        self.conn.execute(
            "INSERT INTO withdrawals (commitment, note_id, amount, block_number, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(commitment) DO NOTHING",
            rusqlite::params![commitment, note_id, amount, block_number, created_at],
        )?;
        
        Ok(())
    }

    /// Get withdrawal by commitment
    pub fn get_withdrawal(&self, commitment: &str) -> SqlResult<Option<WithdrawalRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT commitment, note_id, amount, block_number, created_at, claimed_at, zcash_txid
             FROM withdrawals WHERE commitment = ?1"
        )?;
        
        let mut rows = stmt.query_map([commitment], |row| {
            Ok(WithdrawalRecord {
                commitment: row.get(0)?,
                note_id: row.get(1)?,
                amount: row.get(2)?,
                block_number: row.get(3)?,
                created_at: row.get(4)?,
                claimed_at: row.get(5)?,
                zcash_txid: row.get(6)?,
            })
        })?;
        
        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }

    /// Get withdrawal by note_id
    pub fn get_withdrawal_by_note_id(&self, note_id: &str) -> SqlResult<Option<WithdrawalRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT commitment, note_id, amount, block_number, created_at, claimed_at, zcash_txid
             FROM withdrawals WHERE note_id = ?1"
        )?;
        
        let mut rows = stmt.query_map([note_id], |row| {
            Ok(WithdrawalRecord {
                commitment: row.get(0)?,
                note_id: row.get(1)?,
                amount: row.get(2)?,
                block_number: row.get(3)?,
                created_at: row.get(4)?,
                claimed_at: row.get(5)?,
                zcash_txid: row.get(6)?,
            })
        })?;
        
        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }

    /// Mark withdrawal as claimed
    pub fn mark_claimed(
        &self,
        commitment: &str,
        zcash_txid: &str,
    ) -> SqlResult<()> {
        let claimed_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        self.conn.execute(
            "UPDATE withdrawals 
             SET claimed_at = ?1, zcash_txid = ?2
             WHERE commitment = ?3",
            rusqlite::params![claimed_at, zcash_txid, commitment],
        )?;
        
        Ok(())
    }

    /// Check if withdrawal is already claimed
    pub fn is_claimed(&self, commitment: &str) -> SqlResult<bool> {
        let mut stmt = self.conn.prepare(
            "SELECT 1 FROM withdrawals WHERE commitment = ?1 AND claimed_at IS NOT NULL LIMIT 1"
        )?;
        
        let exists = stmt.exists([commitment])?;
        Ok(exists)
    }

    /// Get all unclaimed withdrawals
    pub fn get_unclaimed_withdrawals(&self) -> SqlResult<Vec<WithdrawalRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT commitment, note_id, amount, block_number, created_at, claimed_at, zcash_txid
             FROM withdrawals WHERE claimed_at IS NULL"
        )?;
        
        let rows = stmt.query_map([], |row| {
            Ok(WithdrawalRecord {
                commitment: row.get(0)?,
                note_id: row.get(1)?,
                amount: row.get(2)?,
                block_number: row.get(3)?,
                created_at: row.get(4)?,
                claimed_at: row.get(5)?,
                zcash_txid: row.get(6)?,
            })
        })?;
        
        let mut withdrawals = Vec::new();
        for row in rows {
            withdrawals.push(row?);
        }
        
        Ok(withdrawals)
    }
}

