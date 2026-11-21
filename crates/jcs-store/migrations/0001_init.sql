-- JC Studio initial schema.
--
-- Design notes:
--  * `events` is the append-only source of truth. State tables are projections kept
--    in the same transaction as their event, so replay is always possible.
--  * Event scope columns (project_id/goal_id/task_id/run_id) intentionally have no
--    foreign keys: the audit trail must outlive deletions.
--  * Timestamps are TEXT in RFC 3339 UTC with millisecond precision, so lexicographic
--    ordering equals chronological ordering.
--  * Status columns hold the snake_case wire names validated by jcs-core.

CREATE TABLE projects (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL UNIQUE,
    root_path   TEXT NOT NULL UNIQUE,
    base_branch TEXT NOT NULL DEFAULT 'main',
    created_at  TEXT NOT NULL
);

CREATE TABLE goals (
    id         TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    prompt     TEXT NOT NULL,
    status     TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_goals_project ON goals(project_id, created_at DESC);
CREATE INDEX idx_goals_status ON goals(status);

CREATE TABLE tasks (
    id         TEXT PRIMARY KEY NOT NULL,
    goal_id    TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
    title      TEXT NOT NULL,
    status     TEXT NOT NULL,
    worktree   TEXT,
    branch     TEXT,
    session_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_tasks_goal ON tasks(goal_id, created_at);
CREATE INDEX idx_tasks_status ON tasks(status);

CREATE TABLE runs (
    id         TEXT PRIMARY KEY NOT NULL,
    task_id    TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    attempt    INTEGER NOT NULL,
    status     TEXT NOT NULL,
    exit_code  INTEGER,
    started_at TEXT NOT NULL,
    ended_at   TEXT,
    cost_usd   REAL NOT NULL DEFAULT 0,
    tokens_in  INTEGER NOT NULL DEFAULT 0,
    tokens_out INTEGER NOT NULL DEFAULT 0,
    error      TEXT,
    UNIQUE(task_id, attempt)
);
CREATE INDEX idx_runs_task ON runs(task_id, attempt);

CREATE TABLE events (
    seq        INTEGER PRIMARY KEY AUTOINCREMENT,
    id         TEXT NOT NULL UNIQUE,
    ts         TEXT NOT NULL,
    kind       TEXT NOT NULL,
    project_id TEXT,
    goal_id    TEXT,
    task_id    TEXT,
    run_id     TEXT,
    payload    TEXT NOT NULL DEFAULT 'null'
);
CREATE INDEX idx_events_task ON events(task_id, seq);
CREATE INDEX idx_events_run ON events(run_id, seq);
CREATE INDEX idx_events_kind ON events(kind, seq);

CREATE TABLE gate_defs (
    id           TEXT PRIMARY KEY NOT NULL,
    project_id   TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    command      TEXT NOT NULL,
    timeout_secs INTEGER NOT NULL DEFAULT 300,
    required     INTEGER NOT NULL DEFAULT 1,
    UNIQUE(project_id, name)
);

CREATE TABLE gate_results (
    id             TEXT PRIMARY KEY NOT NULL,
    run_id         TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    gate_id        TEXT NOT NULL,
    name           TEXT NOT NULL,
    status         TEXT NOT NULL,
    exit_code      INTEGER,
    duration_ms    INTEGER NOT NULL DEFAULT 0,
    output_excerpt TEXT,
    created_at     TEXT NOT NULL
);
CREATE INDEX idx_gate_results_run ON gate_results(run_id);

CREATE TABLE leases (
    resource     TEXT PRIMARY KEY NOT NULL,
    task_id      TEXT NOT NULL,
    owner        TEXT NOT NULL,
    acquired_at  TEXT NOT NULL,
    heartbeat_at TEXT NOT NULL,
    expires_at   TEXT NOT NULL
);

CREATE TABLE sessions_map (
    session_id TEXT PRIMARY KEY NOT NULL,
    task_id    TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    directory  TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE schema_meta (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
