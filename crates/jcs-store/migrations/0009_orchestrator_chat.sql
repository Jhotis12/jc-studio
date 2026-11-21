-- Persistent conversation with the orchestrator agent, per project.
CREATE TABLE IF NOT EXISTS orchestrator_messages (
    id         TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    role       TEXT NOT NULL,
    text       TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_orchestrator_messages_project
    ON orchestrator_messages(project_id, created_at);

CREATE TABLE IF NOT EXISTS orchestrator_sessions (
    project_id TEXT PRIMARY KEY NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    session_id TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
