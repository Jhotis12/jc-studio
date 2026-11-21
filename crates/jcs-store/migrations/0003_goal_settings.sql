-- Phase 3: per-goal execution settings.
--
-- The model, agent and permission policy chosen when the objective is created must survive
-- planning and apply to every task the plan produces. Storing them on the goal keeps the
-- scheduler free of client state.

ALTER TABLE goals ADD COLUMN model TEXT;
ALTER TABLE goals ADD COLUMN agent TEXT;
ALTER TABLE goals ADD COLUMN permissions TEXT NOT NULL DEFAULT 'auto';
