-- The planner writes a standalone description for every task. Until now it was used only
-- for the prompt and then discarded; iteration and retries need to persist it so the next
-- run sees the operator's feedback.
ALTER TABLE tasks ADD COLUMN instructions TEXT;
