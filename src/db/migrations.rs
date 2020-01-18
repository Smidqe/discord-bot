use super::Result;
use log::info;
use rusqlite::{Connection, NO_PARAMS};

const MIGRATION_STEPS: u32 = 1;

fn migrate_step(conn: &Connection, step: u32) -> Result<()> {
    info!("Running migration step {}", step);
    match step {
        0 => {
            conn.execute_batch(
                "
                BEGIN;

                CREATE TABLE users (
                    id TEXT PRIMARY KEY NOT NULL,
                    first_online TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    last_online TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    first_message TEXT DEFAULT NULL,
                    last_message TEXT DEFAULT NULL
                ) WITHOUT ROWID;

                CREATE TABLE usernames (
                    id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
                    name TEXT NOT NULL CHECK (length(name) BETWEEN 2 and 32),
                    discriminator TEXT NOT NULL CHECK (length(discriminator) = 4),
                    first_online TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    last_online TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    PRIMARY KEY (id, name, discriminator)
                ) WITHOUT ROWID;

                CREATE TABLE nicks (
                    id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
                    nick TEXT NOT NULL CHECK (length(nick) <= 32),
                    first_online TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    last_online TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    PRIMARY KEY (id, nick)
                ) WITHOUT ROWID;

                COMMIT;
            ",
            )?;
        }
        _ => unreachable!(),
    }
    Ok(())
}

pub fn apply_migrations(conn: &Connection) -> Result<(u32, u32)> {
    let initial: u32 = conn.query_row(
        "SELECT user_version FROM pragma_user_version",
        NO_PARAMS,
        |row| row.get(0),
    )?;
    for step in initial..MIGRATION_STEPS {
        migrate_step(&conn, step)?;
        conn.execute_batch(&format!("PRAGMA user_version = {}", step + 1))?;
    }
    Ok((initial, MIGRATION_STEPS - 1))
}