-- Ongoing operations of the applications and of the system: a few rows, pruned, read whole.
-- The id is chosen by the owner so that restarted work finds its row again.
-- A null progress means unknown.
CREATE TABLE tasks (
    id          TEXT    NOT NULL PRIMARY KEY,
    source      TEXT    NOT NULL,
    name        TEXT    NOT NULL,
    detail      TEXT    NOT NULL DEFAULT '',
    progress    INTEGER CHECK (progress IS NULL OR (progress BETWEEN 0 AND 100)),
    state       TEXT    NOT NULL CHECK (state IN ('running', 'paused', 'done', 'failed', 'cancelled')),
    pausable    INTEGER NOT NULL DEFAULT 0 CHECK (pausable IN (0, 1)),
    cancellable INTEGER NOT NULL DEFAULT 0 CHECK (cancellable IN (0, 1)),
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
) STRICT;
