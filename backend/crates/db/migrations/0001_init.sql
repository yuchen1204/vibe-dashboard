CREATE TABLE IF NOT EXISTS schema_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT INTO schema_meta(key, value) VALUES('schema_version', '1')
    ON CONFLICT(key) DO UPDATE SET value = excluded.value;

INSERT INTO schema_meta(key, value) VALUES('created_at', strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    ON CONFLICT(key) DO NOTHING;
