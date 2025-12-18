use rusqlite::{params, Connection};
use std::sync::{Arc, Mutex};

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
    
    /// Find all other message instances associated with this one (Source <=> Dests)
    /// Returns Vec<(dest_service, dest_channel, dest_id)>
    pub fn get_associated_mappings(
        &self,
        service: &str,
        channel: &str,
        message_id: &str,
    ) -> anyhow::Result<Vec<(String, String, String)>> {
        let conn = self.conn.lock().unwrap();
        
        // Find the canonical source for this message ID (it could be the source OR a destination)
        let mut stmt = conn.prepare(
            "SELECT DISTINCT source_service, source_channel, source_id FROM message_map 
             WHERE (source_service = ?1 AND source_id = ?2) 
                OR (dest_service = ?1 AND dest_id = ?2)"
        )?;
        
        let canon = stmt.query_row(params![service, message_id], |row| {
           Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        });

        match canon {
            Ok((c_svc, _c_chan, c_id)) => {
                let mut stmt_all = conn.prepare(
                    "SELECT source_service, source_channel, source_id, dest_service, dest_channel, dest_id 
                     FROM message_map 
                     WHERE source_service = ?1 AND source_id = ?2"
                )?;
                
                let rows = stmt_all.query_map(params![c_svc, c_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, // source
                        row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?, // dest
                    ))
                })?;

                let mut set = std::collections::HashSet::new();
                for row in rows {
                    let r = row?;
                    // Add source
                    set.insert((r.0, r.1, r.2));
                    // Add dest
                    set.insert((r.3, r.4, r.5));
                }

                // Remove the one we started with
                set.remove(&(service.to_string(), channel.to_string(), message_id.to_string()));

                Ok(set.into_iter().collect())
            },
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(vec![]),
            Err(e) => Err(anyhow::anyhow!("Lookup error: {}", e)),
        }
    }
}
