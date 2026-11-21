//! Durable orchestrator conversation.
//!
//! The agent session lives in OpenCode; the transcript lives here so closing the app or
//! restarting the daemon does not lose what the operator asked.

use jiff::Timestamp;

use crate::error::Result;
use crate::store::{decode_ts, encode_timestamp, Store};
use jcs_core::ProjectId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchestratorMessage {
    pub role: String,
    pub text: String,
    pub created_at: Timestamp,
}

impl Store {
    pub async fn orchestrator_messages(
        &self,
        project_id: &ProjectId,
        limit: i64,
    ) -> Result<Vec<OrchestratorMessage>> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT role, text, created_at FROM ( \
                 SELECT rowid AS rid, role, text, created_at FROM orchestrator_messages \
                 WHERE project_id = ? ORDER BY created_at DESC, rowid DESC LIMIT ? \
             ) ORDER BY created_at ASC, rid ASC",
        )
        .bind(project_id.as_str())
        .bind(limit.clamp(1, 500))
        .fetch_all(self.pool())
        .await?;

        rows.into_iter()
            .map(|(role, text, created_at)| {
                Ok(OrchestratorMessage {
                    role,
                    text,
                    created_at: decode_ts(&created_at)?,
                })
            })
            .collect()
    }

    pub async fn append_orchestrator_message(
        &self,
        project_id: &ProjectId,
        role: &str,
        text: &str,
    ) -> Result<OrchestratorMessage> {
        let message = OrchestratorMessage {
            role: role.to_string(),
            text: text.to_string(),
            created_at: Timestamp::now(),
        };
        sqlx::query(
            "INSERT INTO orchestrator_messages (id, project_id, role, text, created_at) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(project_id.as_str())
        .bind(&message.role)
        .bind(&message.text)
        .bind(encode_timestamp(message.created_at))
        .execute(self.pool())
        .await?;
        Ok(message)
    }

    pub async fn orchestrator_session(&self, project_id: &ProjectId) -> Result<Option<String>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT session_id FROM orchestrator_sessions WHERE project_id = ?")
                .bind(project_id.as_str())
                .fetch_optional(self.pool())
                .await?;
        Ok(row.map(|(session,)| session))
    }

    pub async fn set_orchestrator_session(
        &self,
        project_id: &ProjectId,
        session_id: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO orchestrator_sessions (project_id, session_id, updated_at) \
             VALUES (?, ?, ?) ON CONFLICT(project_id) DO UPDATE SET \
             session_id = excluded.session_id, updated_at = excluded.updated_at",
        )
        .bind(project_id.as_str())
        .bind(session_id)
        .bind(encode_timestamp(Timestamp::now()))
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn clear_orchestrator(&self, project_id: &ProjectId) -> Result<()> {
        sqlx::query("DELETE FROM orchestrator_messages WHERE project_id = ?")
            .bind(project_id.as_str())
            .execute(self.pool())
            .await?;
        sqlx::query("DELETE FROM orchestrator_sessions WHERE project_id = ?")
            .bind(project_id.as_str())
            .execute(self.pool())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jcs_core::{Project, ProjectId};

    #[tokio::test]
    async fn the_transcript_and_session_are_durable_and_clearable() {
        let store = Store::in_memory().await.unwrap();
        let project = Project {
            id: ProjectId::new(),
            name: "chat".to_string(),
            root_path: std::env::temp_dir().join("jcs-chat"),
            base_branch: "main".to_string(),
            created_at: Timestamp::now(),
            auto_merge: false,
            budget_usd_per_day: None,
            budget_tokens_per_day: None,
        };
        store.insert_project(&project).await.unwrap();

        store
            .append_orchestrator_message(&project.id, "user", "hola")
            .await
            .unwrap();
        store
            .append_orchestrator_message(&project.id, "assistant", "¿qué quieres hacer?")
            .await
            .unwrap();
        let messages = store.orchestrator_messages(&project.id, 10).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].text, "¿qué quieres hacer?");

        store
            .set_orchestrator_session(&project.id, "ses_test")
            .await
            .unwrap();
        assert_eq!(
            store
                .orchestrator_session(&project.id)
                .await
                .unwrap()
                .as_deref(),
            Some("ses_test")
        );

        store.clear_orchestrator(&project.id).await.unwrap();
        assert!(store
            .orchestrator_messages(&project.id, 10)
            .await
            .unwrap()
            .is_empty());
        assert!(store
            .orchestrator_session(&project.id)
            .await
            .unwrap()
            .is_none());
    }
}
