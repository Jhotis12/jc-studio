-- OpenCode reports tokens but can report cost 0 (subscription, credits, or a provider that
-- does not price the usage). The catalog does know the rate, so the daemon stores it per
-- run and computes the cost from tokens when the backend does not.
ALTER TABLE runs ADD COLUMN model TEXT;
ALTER TABLE runs ADD COLUMN cost_input_per_mtok REAL NOT NULL DEFAULT 0;
ALTER TABLE runs ADD COLUMN cost_output_per_mtok REAL NOT NULL DEFAULT 0;
