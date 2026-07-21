CREATE TABLE instance_settings (
    id INTEGER PRIMARY KEY,
    version INTEGER NOT NULL,
    settings_json LONGTEXT NOT NULL,
    credentials_encrypted LONGTEXT,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CHECK (id = 1)
);
