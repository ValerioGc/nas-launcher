-- First configuration of the NAS. The row exists once the setup wizard is completed.
CREATE TABLE setup (
    id           INTEGER PRIMARY KEY CHECK (id = 1),
    completed_at INTEGER NOT NULL
) STRICT;

-- Steps run by the launcher after the wizard: the user of the terminal, the data volumes, the
-- release token and the preinstalled applications.
CREATE TABLE setup_steps (
    id         TEXT    PRIMARY KEY,
    position   INTEGER NOT NULL,
    kind       TEXT    NOT NULL CHECK (kind IN ('account', 'storage', 'token', 'app')),
    name       TEXT    NOT NULL,
    state      TEXT    NOT NULL CHECK (state IN ('pending', 'running', 'done', 'failed')),
    detail     TEXT    NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL
) STRICT;
