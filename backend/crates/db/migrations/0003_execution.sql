CREATE TABLE worktrees (
    id           TEXT NOT NULL PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    target_id    TEXT,
    branch       TEXT NOT NULL,
    path         TEXT NOT NULL,
    status       TEXT NOT NULL DEFAULT 'active'
                 CHECK (status IN ('active', 'merged', 'abandoned')),
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE TABLE execution_jobs (
    id          TEXT NOT NULL PRIMARY KEY,
    todo_id     TEXT NOT NULL,
    worktree_id TEXT,
    status      TEXT NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending', 'running', 'success', 'failed', 'cancelled')),
    agent_type  TEXT NOT NULL DEFAULT 'claude-code',
    prompt      TEXT NOT NULL,
    output      TEXT NOT NULL DEFAULT '',
    started_at  TEXT,
    finished_at TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    FOREIGN KEY (todo_id) REFERENCES todos(id) ON DELETE CASCADE
);

CREATE INDEX idx_worktrees_workspace ON worktrees(workspace_id);
CREATE INDEX idx_jobs_todo          ON execution_jobs(todo_id);
CREATE INDEX idx_jobs_status        ON execution_jobs(status);