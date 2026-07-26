CREATE TABLE IF NOT EXISTS settings (
    key   TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT INTO schema_meta(key, value) VALUES('schema_version', '4')
    ON CONFLICT(key) DO UPDATE SET value = excluded.value;