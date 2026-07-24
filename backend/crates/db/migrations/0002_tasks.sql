CREATE TABLE workspaces (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    path       TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE targets (
    id          TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL DEFAULT 'planned'
                CHECK (status IN ('planned', 'active', 'done', 'archived')),
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE TABLE todos (
    id          TEXT PRIMARY KEY,
    target_id   TEXT NOT NULL,
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL DEFAULT 'todo'
                CHECK (status IN ('todo', 'doing', 'done', 'blocked')),
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    FOREIGN KEY (target_id) REFERENCES targets(id) ON DELETE CASCADE
);

CREATE INDEX idx_targets_workspace ON targets(workspace_id);
CREATE INDEX idx_todos_target      ON todos(target_id);
CREATE INDEX idx_todos_status      ON todos(status);
