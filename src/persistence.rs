use rusqlite::{params, Connection, Result};
use std::sync::{Arc, Mutex};
use log::{info, error};

/// Manages persistent storage for message ID mapping
#[derive(Clone)]
pub struct MessageStore {
    conn: Arc<Mutex<Connection>>,
}

impl MessageStore {
    pub fn new(path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        
        // Create table if not exists
        conn.execute(
            "CREATE TABLE IF NOT EXISTS message_map (
                source_service TEXT NOT NULL,
                source_channel TEXT NOT NULL,
                source_id TEXT NOT NULL,
                dest_service TEXT NOT NULL,
                dest_channel TEXT NOT NULL,
                dest_id TEXT NOT NULL,
                timestamp INTEGER DEFAULT (strftime('%s', 'now')),
                PRIMARY KEY (source_service, source_channel, source_id, dest_service)
            )",
            [],
        )?;
        
        // Index for fast lookups by source ID
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_source ON message_map(source_service, source_channel, source_id)",
            [],
        )?;
        
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
    
    /// Check if a message mapping exists for the given source
    pub fn exists(&self, source_service: &str, source_channel: &str, source_id: &str) -> anyhow::Result<bool> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT 1 FROM message_map 
            WHERE source_service = ?1 AND source_channel = ?2 AND source_id = ?3 
            LIMIT 1"
        )?;
        
        let exists = stmt.exists(params![source_service, source_channel, source_id])?;
        Ok(exists)
    }

    /// Save a mapping from a source message to a bridged message
    pub fn save_mapping(
        &self,
        source_service: &str,
        source_channel: &str,
        source_id: &str,
        dest_service: &str,
        dest_channel: &str,
        dest_id: &str,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO message_map 
            (source_service, source_channel, source_id, dest_service, dest_channel, dest_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![source_service, source_channel, source_id, dest_service, dest_channel, dest_id],
        )?;
        Ok(())
    }
    
    /// Find all destination messages for a given source message
    /// Returns Vec<(dest_service, dest_channel, dest_id)>
    pub fn get_mappings(
        &self,
        source_service: &str,
        source_channel: &str,
        source_id: &str,
    ) -> anyhow::Result<Vec<(String, String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT dest_service, dest_channel, dest_id FROM message_map 
            WHERE source_service = ?1 AND source_channel = ?2 AND source_id = ?3"
        )?;
        
        let rows = stmt.query_map(params![source_service, source_channel, source_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
            ))
        })?;
        
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        
        Ok(result)
    }
}
