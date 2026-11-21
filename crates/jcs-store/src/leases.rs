//! Leases: who owns a piece of work right now.
//!
//! Leases are the mechanism that makes "run it again after a crash" safe. A task is only
//! dispatched when the scheduler can acquire its lease, and the holder must renew it. An
//! expired lease is reclaimed, which is exactly what should happen after `kill -9`.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::store::Store;
use jcs_core::{NewEvent, TaskId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lease {
    pub resource: String,
    pub task_id: TaskId,
    pub owner: String,
    pub acquired_at: Timestamp,
    pub heartbeat_at: Timestamp,
    pub expires_at: Timestamp,
}

impl Lease {
    pub fn is_expired(&self, now: Timestamp) -> bool {
        self.expires_at < now
    }
}

impl Store {
    /// Tries to take a lease. Returns true only if this owner now holds it.
    ///
    /// Expired leases are removed first, so a crashed holder never blocks work forever.
    pub async fn acquire_lease(
        &self,
        resource: &str,
        task_id: &TaskId,
        owner: &str,
        ttl: std::time::Duration,
    ) -> Result<bool> {
        let now = Timestamp::now();
        let expires = Timestamp::from_millisecond(now.as_millisecond() + ttl.as_millis() as i64)
            .unwrap_or(now);

        let mut tx = self.pool().begin().await?;

        sqlx::query("DELETE FROM leases WHERE resource = ? AND expires_at < ?")
            .bind(resource)
            .bind(crate::store::encode_ts(now))
            .execute(&mut *tx)
            .await?;

        let inserted = sqlx::query(
            "INSERT OR IGNORE INTO leases \
             (resource, task_id, owner, acquired_at, heartbeat_at, expires_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(resource)
        .bind(task_id.as_str())
        .bind(owner)
        .bind(crate::store::encode_ts(now))
        .bind(crate::store::encode_ts(now))
        .bind(crate::store::encode_ts(expires))
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(inserted.rows_affected() == 1)
    }

    /// Extends a lease this owner already holds. Returns false if it was lost.
    pub async fn renew_lease(
        &self,
        resource: &str,
        owner: &str,
        ttl: std::time::Duration,
    ) -> Result<bool> {
        let now = Timestamp::now();
        let expires = Timestamp::from_millisecond(now.as_millisecond() + ttl.as_millis() as i64)
            .unwrap_or(now);

        let result = sqlx::query(
            "UPDATE leases SET heartbeat_at = ?, expires_at = ? \
             WHERE resource = ? AND owner = ?",
        )
        .bind(crate::store::encode_ts(now))
        .bind(crate::store::encode_ts(expires))
        .bind(resource)
        .bind(owner)
        .execute(self.pool())
        .await?;

        Ok(result.rows_affected() == 1)
    }

    pub async fn release_lease(&self, resource: &str, owner: &str) -> Result<()> {
        sqlx::query("DELETE FROM leases WHERE resource = ? AND owner = ?")
            .bind(resource)
            .bind(owner)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Removes every expired lease. Called by the scheduler on each tick.
    pub async fn expire_leases(&self) -> Result<u64> {
        let now = crate::store::encode_ts(Timestamp::now());
        let result = sqlx::query("DELETE FROM leases WHERE expires_at < ?")
            .bind(now)
            .execute(self.pool())
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn lease_holder(&self, resource: &str) -> Result<Option<Lease>> {
        let row: Option<(String, String, String, String, String, String)> = sqlx::query_as(
            "SELECT resource, task_id, owner, acquired_at, heartbeat_at, expires_at \
             FROM leases WHERE resource = ?",
        )
        .bind(resource)
        .fetch_optional(self.pool())
        .await?;

        row.map(
            |(resource, task_id, owner, acquired_at, heartbeat_at, expires_at)| {
                Ok(Lease {
                    resource,
                    task_id: TaskId::parse(task_id)?,
                    owner,
                    acquired_at: crate::store::decode_ts(&acquired_at)?,
                    heartbeat_at: crate::store::decode_ts(&heartbeat_at)?,
                    expires_at: crate::store::decode_ts(&expires_at)?,
                })
            },
        )
        .transpose()
    }

    pub async fn all_leases(&self) -> Result<Vec<Lease>> {
        let rows: Vec<(String, String, String, String, String, String)> = sqlx::query_as(
            "SELECT resource, task_id, owner, acquired_at, heartbeat_at, expires_at \
             FROM leases ORDER BY resource",
        )
        .fetch_all(self.pool())
        .await?;

        rows.into_iter()
            .map(
                |(resource, task_id, owner, acquired_at, heartbeat_at, expires_at)| {
                    Ok(Lease {
                        resource,
                        task_id: TaskId::parse(task_id)?,
                        owner,
                        acquired_at: crate::store::decode_ts(&acquired_at)?,
                        heartbeat_at: crate::store::decode_ts(&heartbeat_at)?,
                        expires_at: crate::store::decode_ts(&expires_at)?,
                    })
                },
            )
            .collect()
    }

    /// Records that the scheduler reclaimed expired leases.
    pub async fn note_expired_leases(&self, count: u64) -> Result<()> {
        if count == 0 {
            return Ok(());
        }
        let event = NewEvent::new(jcs_core::EventKind::Error).payload(serde_json::json!({
            "category": "expired_leases_reclaimed",
            "count": count,
            "hint": "a previous owner stopped renewing its lease; its work is being retried",
        }));
        crate::append_event(self, event).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jcs_core::{EventKind, ProjectId};
    use std::time::Duration;

    async fn fixture() -> (Store, TaskId) {
        let store = Store::in_memory().await.unwrap();
        let project = jcs_core::Project {
            id: ProjectId::new(),
            name: "demo".into(),
            root_path: std::path::PathBuf::from("/tmp/demo"),
            base_branch: "main".into(),
            created_at: Timestamp::now(),
            auto_merge: false,
            budget_usd_per_day: None,
            budget_tokens_per_day: None,
        };
        store.insert_project(&project).await.unwrap();
        let goal = store.create_goal(&project.id, "goal").await.unwrap();
        let task = store.create_task(&goal.id, "task").await.unwrap();
        (store, task.id)
    }

    #[tokio::test]
    async fn a_lease_can_be_taken_only_once() {
        let (store, task) = fixture().await;

        assert!(store
            .acquire_lease(
                &format!("task:{task}"),
                &task,
                "owner-a",
                Duration::from_secs(60)
            )
            .await
            .unwrap());
        assert!(!store
            .acquire_lease(
                &format!("task:{task}"),
                &task,
                "owner-b",
                Duration::from_secs(60)
            )
            .await
            .unwrap());

        let holder = store
            .lease_holder(&format!("task:{task}"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(holder.owner, "owner-a");
    }

    #[tokio::test]
    async fn an_expired_lease_is_reclaimed() {
        let (store, task) = fixture().await;
        let resource = format!("task:{task}");

        assert!(store
            .acquire_lease(&resource, &task, "dead-owner", Duration::from_millis(1))
            .await
            .unwrap());
        tokio::time::sleep(Duration::from_millis(10)).await;

        assert!(store
            .acquire_lease(&resource, &task, "new-owner", Duration::from_secs(60))
            .await
            .unwrap());
        assert_eq!(
            store.lease_holder(&resource).await.unwrap().unwrap().owner,
            "new-owner"
        );
    }

    #[tokio::test]
    async fn only_the_holder_can_renew_or_release() {
        let (store, task) = fixture().await;
        let resource = format!("task:{task}");
        store
            .acquire_lease(&resource, &task, "owner-a", Duration::from_secs(60))
            .await
            .unwrap();

        assert!(!store
            .renew_lease(&resource, "owner-b", Duration::from_secs(60))
            .await
            .unwrap());
        assert!(store
            .renew_lease(&resource, "owner-a", Duration::from_secs(60))
            .await
            .unwrap());

        store.release_lease(&resource, "owner-b").await.unwrap();
        assert!(store.lease_holder(&resource).await.unwrap().is_some());
        store.release_lease(&resource, "owner-a").await.unwrap();
        assert!(store.lease_holder(&resource).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn expiring_leases_is_batched_and_audited() {
        let (store, task) = fixture().await;
        store
            .acquire_lease(
                &format!("task:{task}"),
                &task,
                "gone",
                Duration::from_millis(1),
            )
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        let removed = store.expire_leases().await.unwrap();
        assert_eq!(removed, 1);
        store.note_expired_leases(removed).await.unwrap();

        let events = crate::events_since(&store, 0, 100).await.unwrap();
        assert!(events.iter().any(|event| event.kind == EventKind::Error
            && event.payload["category"] == "expired_leases_reclaimed"));
    }
}
