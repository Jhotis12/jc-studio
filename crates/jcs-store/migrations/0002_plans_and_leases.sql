-- Phase 3: plans, task DAG, leases and budgets.
--
-- Tasks gain a role, a priority, an ordering position and the paths they claim. Claims
-- let the scheduler run independent tasks in parallel without two agents editing the same
-- files. Dependencies live in their own table so readiness is a query, not a graph walk.

ALTER TABLE tasks ADD COLUMN role TEXT NOT NULL DEFAULT 'implement';
ALTER TABLE tasks ADD COLUMN priority INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tasks ADD COLUMN path_claims TEXT NOT NULL DEFAULT '[]';
ALTER TABLE tasks ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

CREATE TABLE task_deps (
    task_id    TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    depends_on TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, depends_on)
);
CREATE INDEX idx_task_deps_dependency ON task_deps(depends_on);

-- The planner's validated output, kept verbatim for audit and re-planning.
CREATE TABLE plans (
    goal_id    TEXT PRIMARY KEY REFERENCES goals(id) ON DELETE CASCADE,
    version    INTEGER NOT NULL,
    raw        TEXT NOT NULL,
    created_at TEXT NOT NULL
);

ALTER TABLE projects ADD COLUMN auto_merge INTEGER NOT NULL DEFAULT 0;
ALTER TABLE projects ADD COLUMN budget_usd_per_day REAL;
ALTER TABLE projects ADD COLUMN budget_tokens_per_day INTEGER;

CREATE INDEX idx_runs_started ON runs(started_at);
