use rusqlite::{Connection, Result as SqlResult};
use miden_objects::account::AccountId;
use miden_objects::utils::{Deserializable, Serializable};
use std::path::PathBuf;

pub struct FaucetStore {
    conn: Connection,
}

impl FaucetStore {
    pub fn new(db_path: PathBuf) -> SqlResult<Self> {
        let conn = Connection::open(db_path)?;
        
        // Create faucets table if it doesn't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS faucets (
                origin_network TEXT PRIMARY KEY,
                faucet_id BLOB NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        )?;
        
        Ok(Self { conn })
    }

    /// Get faucet_id for a given origin network
    pub fn get_faucet_id(&self, origin_network: &str) -> SqlResult<Option<AccountId>> {
        let mut stmt = self.conn.prepare(
            "SELECT faucet_id FROM faucets WHERE origin_network = ?1"
        )?;
        
        let mut rows = stmt.query_map([origin_network], |row| {
            let faucet_id_bytes: Vec<u8> = row.get(0)?;
            AccountId::read_from_bytes(&faucet_id_bytes)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    faucet_id_bytes.len(),
                    rusqlite::types::Type::Blob,
                    Box::new(e)
                ))
        })?;
        
        match rows.next() {
            Some(Ok(faucet_id)) => Ok(Some(faucet_id)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    /// Store faucet_id for a given origin network
    pub fn store_faucet_id(&self, origin_network: &str, faucet_id: &AccountId) -> SqlResult<()> {
        let faucet_id_bytes = faucet_id.to_bytes();
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        self.conn.execute(
            "INSERT OR REPLACE INTO faucets (origin_network, faucet_id, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![origin_network, faucet_id_bytes, created_at],
        )?;
        
        Ok(())
    }
}

