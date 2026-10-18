-- Notification log, read from the most recent one: the id is the order, never reused (a link to a
-- notification names its id). The key and the params let the interface translate a notification
-- of the launcher; the log is the detailed information about it.
CREATE TABLE notifications (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    source     TEXT    NOT NULL,
    title      TEXT    NOT NULL,
    message    TEXT    NOT NULL,
    tone       TEXT    NOT NULL CHECK (tone IN ('info', 'success', 'warning', 'error')),
    read       INTEGER NOT NULL DEFAULT 0 CHECK (read IN (0, 1)),
    created_at INTEGER NOT NULL,
    key        TEXT,
    params     TEXT    NOT NULL DEFAULT '{}',
    log        TEXT
) STRICT;

CREATE INDEX notifications_by_source ON notifications (source, id DESC);
