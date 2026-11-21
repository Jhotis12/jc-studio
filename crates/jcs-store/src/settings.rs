//! Durable runtime settings.
//!
//! Values are stored one row per key. The daemon owns their meaning; this module only
//! persists and returns them, so a corrupt value can be diagnosed with `sqlite3` instead
//! of by reading a serialized blob.

use jiff::Timestamp;

use crate::error::Result;
use crate::store::{encode_timestamp, Store};

impl Store {
    /// Reads one setting, if it was ever written.
    pub async fn setting(&self, key: &str) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(self.pool())
            .await?;
        Ok(row.map(|(value,)| value))
    }

    /// Inserts or replaces one setting.
    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, ?) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, \
             updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value)
        .bind(encode_timestamp(Timestamp::now()))
        .execute(self.pool())
        .await?;
        Ok(())
    }
}
