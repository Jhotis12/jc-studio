-- Runtime settings changed from the interface.
--
-- One row per setting so a value is inspectable without parsing a blob. Secrets are never
-- stored here: they belong to the OS keyring, not to the database.
CREATE TABLE IF NOT EXISTS settings (
    key        TEXT PRIMARY KEY NOT NULL,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
