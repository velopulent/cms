CREATE TABLE instance_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    version INTEGER NOT NULL,
    settings_json TEXT NOT NULL,
    credentials_encrypted TEXT,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
