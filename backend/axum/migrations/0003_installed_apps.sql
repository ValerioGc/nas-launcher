-- Installed external applications. System applications are compiled into the launcher and are not
-- listed here.
-- An empty version means unknown. `background` is whether the application has work to do without
-- its window, as it last said itself: the launcher starts it at boot and leaves it running when its
-- window closes. Only an application whose manifest declares `run.background` may set it.
CREATE TABLE installed_apps (
    id           TEXT    NOT NULL PRIMARY KEY,
    name         TEXT    NOT NULL,
    info         TEXT    NOT NULL,
    version      TEXT    NOT NULL DEFAULT '',
    installed_at INTEGER NOT NULL,
    background   INTEGER NOT NULL DEFAULT 0 CHECK (background IN (0, 1))
) STRICT;

-- Logos of the catalogue applications uploaded from the App Manager.
CREATE TABLE app_logos (
    id         TEXT    NOT NULL PRIMARY KEY,
    mime       TEXT    NOT NULL,
    data       BLOB    NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;
