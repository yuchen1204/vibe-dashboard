CREATE TABLE IF NOT EXISTS reviews (
    id          TEXT NOT NULL PRIMARY KEY,
    job_id      TEXT NOT NULL,
    todo_id     TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending', 'in_progress', 'completed', 'failed')),
    summary     TEXT NOT NULL DEFAULT '',
    score       INTEGER,
    total_findings INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    completed_at TEXT,
    FOREIGN KEY (job_id) REFERENCES execution_jobs(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS review_findings (
    id          TEXT NOT NULL PRIMARY KEY,
    review_id   TEXT NOT NULL,
    severity    TEXT NOT NULL DEFAULT 'minor'
                CHECK (severity IN ('critical', 'major', 'minor', 'suggestion')),
    file_path   TEXT NOT NULL,
    line_number INTEGER,
    category    TEXT NOT NULL DEFAULT 'other',
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    suggestion  TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL,
    FOREIGN KEY (review_id) REFERENCES reviews(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_reviews_job ON reviews(job_id);
CREATE INDEX IF NOT EXISTS idx_reviews_todo ON reviews(todo_id);
CREATE INDEX IF NOT EXISTS idx_findings_review ON review_findings(review_id);

INSERT INTO schema_meta(key, value) VALUES('schema_version', '5')
    ON CONFLICT(key) DO UPDATE SET value = excluded.value;