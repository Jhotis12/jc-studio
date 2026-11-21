-- Phase 3: goal integration branches and task merge state.
--
-- A DAG cannot be correct if every task starts from the original base: a dependent task
-- would never see its predecessor's work. Each goal therefore gets an integration branch,
-- task worktrees are cut from it, and succeeded tasks are merged into it in order.
--
-- `auto_merge` on the project only controls the final promotion of the integration branch
-- into the base branch; merging into the integration branch is always required.

ALTER TABLE goals ADD COLUMN integration_branch TEXT;
ALTER TABLE tasks ADD COLUMN merged_at TEXT;
ALTER TABLE tasks ADD COLUMN merge_error TEXT;

CREATE INDEX idx_tasks_merge ON tasks(status, merged_at);
