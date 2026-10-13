-- Addresses allowed to reach the NAS, as a single address or a CIDR range.
-- An empty table restricts nothing.
-- The value is stored normalised so that the same rule cannot be written twice.
CREATE TABLE access_rules (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    kind       TEXT    NOT NULL CHECK (kind IN ('ip', 'range')),
    value      TEXT    NOT NULL,
    label      TEXT    NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    UNIQUE (kind, value)
) STRICT;
