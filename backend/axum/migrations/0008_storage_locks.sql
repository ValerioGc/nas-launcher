-- Data volumes of removable disks marked as not to be ejected, by the UUID of their filesystem.
CREATE TABLE storage_locks (
    uuid       TEXT    PRIMARY KEY CHECK (length(uuid) BETWEEN 1 AND 64),
    created_at INTEGER NOT NULL
) STRICT;
