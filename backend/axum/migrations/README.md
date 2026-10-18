# Database migrations

Every `<version>_<description>.sql` file is a migration. `sqlx::migrate!` includes them in the
binary at build time and the backend applies them in order at startup, each in its own
transaction. The `_sqlx_migrations` table records the ones that were applied, with a checksum.
This file is ignored, because its name does not have that shape.

- **Name:** a four-digit sequence number and a snake_case description, for example
  `0001_settings.sql`. One migration for each element of the schema, in the order they depend on
  each other.
- **A migration that has been applied is never changed again.** If the checksum does not match,
  the backend refuses to start. A correction is always a new migration. In development, before a
  migration has reached a real NAS, it can still be changed by deleting `backend/data/launcher.db`.
- **Forward only**, with no `.down.sql` file: before updating a NAS, the update script stops the
  launcher and saves the database with the rest of the installation.
- **`STRICT` tables**, with `CHECK` on the values that are allowed and dates as `INTEGER` (Unix
  milliseconds).
- **Short comments**, impersonal, in plain sentences.
- **Changes `ALTER TABLE` does not support** (the type of a column, a constraint): a new table, a
  copy of the data, a `DROP` of the old one and a `RENAME`, all in the same migration.
