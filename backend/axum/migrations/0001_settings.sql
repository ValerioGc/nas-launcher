-- Settings split by section, one row for each section of each user.
-- The position is the order of the sections in the interface and the value is the JSON document of
-- the section, always read by its key.
CREATE TABLE settings (
    user_name  TEXT    NOT NULL,
    section    TEXT    NOT NULL,
    position   INTEGER NOT NULL,
    value      TEXT    NOT NULL CHECK (json_valid(value)),
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (user_name, section)
) STRICT;
