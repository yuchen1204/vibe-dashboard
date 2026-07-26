-- L6 反馈闭环层

-- review_feedback: finding → todo 映射
-- 记录每个 review finding 被如何处理
CREATE TABLE IF NOT EXISTS review_feedback (
    id          TEXT NOT NULL PRIMARY KEY,
    review_id   TEXT NOT NULL,
    finding_id  TEXT NOT NULL,
    todo_id     TEXT,
    action      TEXT NOT NULL DEFAULT 'pending'
                CHECK (action IN ('pending', 'accepted', 'ignored', 'auto_fix')),
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    FOREIGN KEY (review_id) REFERENCES reviews(id) ON DELETE CASCADE,
    FOREIGN KEY (finding_id) REFERENCES review_findings(id) ON DELETE CASCADE
);

-- review_iterations: 迭代循环记录
-- 每次执行→审查构成一次迭代
CREATE TABLE IF NOT EXISTS review_iterations (
    id          TEXT NOT NULL PRIMARY KEY,
    todo_id     TEXT NOT NULL,
    iteration   INTEGER NOT NULL DEFAULT 1,
    job_id      TEXT,
    review_id   TEXT,
    status      TEXT NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending', 'running', 'passed', 'failed', 'maxed_out')),
    score       INTEGER,
    threshold   INTEGER NOT NULL DEFAULT 8,
    summary     TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    FOREIGN KEY (todo_id) REFERENCES todos(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_feedback_review ON review_feedback(review_id);
CREATE INDEX IF NOT EXISTS idx_feedback_todo ON review_feedback(todo_id);
CREATE INDEX IF NOT EXISTS idx_iterations_todo ON review_iterations(todo_id);

INSERT INTO schema_meta(key, value) VALUES('schema_version', '6')
    ON CONFLICT(key) DO UPDATE SET value = excluded.value;