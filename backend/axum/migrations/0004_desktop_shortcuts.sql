-- Desktop shortcuts of each user.
-- The target is the application id or, for a folder, its path. The name is used by folders only.
CREATE TABLE desktop_shortcuts (
    user_name  TEXT    NOT NULL,
    kind       TEXT    NOT NULL CHECK (kind IN ('app', 'folder')),
    target     TEXT    NOT NULL,
    name       TEXT,
    position   INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (user_name, kind, target)
) STRICT;

CREATE INDEX desktop_shortcuts_by_position ON desktop_shortcuts (user_name, position);

-- Starting desktop with the system applications, written once so that a removed icon does not come
-- back. The terminal is opened from the menu: it has no icon until somebody adds one.
INSERT INTO desktop_shortcuts (user_name, kind, target, name, position, created_at)
SELECT 'local', 'app', app.target, NULL, app.position, CAST(strftime('%s', 'now') AS INTEGER) * 1000
FROM (
    SELECT 'nas-app-manager' AS target, 0 AS position
    UNION ALL SELECT 'nas-global-search', 1
    UNION ALL SELECT 'nas-notification-manager', 2
    UNION ALL SELECT 'nas-system-settings', 3
) AS app;
