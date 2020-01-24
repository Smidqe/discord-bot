use super::Result;
use log::info;
use rusqlite::{Connection, NO_PARAMS};

fn migrate_step(conn: &Connection, step: u32) -> Result<()> {
    info!("Running migration step {}", step);
    match step {
        0 => conn.execute_batch(include_str!("migrations/0.sql"))?,
        1 => conn.execute_batch(include_str!("migrations/1.sql"))?,
        2 => conn.execute_batch(include_str!("migrations/2.sql"))?,
        3 => conn.execute_batch(include_str!("migrations/3.sql"))?,
        MIGRATION_STEPS => unreachable!(),
        _ => unreachable!(),
    }
    Ok(())
}

const MIGRATION_STEPS: u32 = 4;

#[inline]
fn tracer(s: &str) {
    log::trace!("SQL: {}", s);
}

pub fn apply_migrations(conn: &mut Connection) -> Result<(u32, u32)> {
    conn.trace(Some(tracer));
    let initial: u32 = conn.query_row(
        "SELECT user_version FROM pragma_user_version",
        NO_PARAMS,
        |row| row.get(0),
    )?;
    for step in initial..MIGRATION_STEPS {
        migrate_step(&conn, step)?;
        conn.execute_batch(&format!("PRAGMA user_version = {}", step + 1))?;
    }
    conn.trace(None);
    Ok((initial, MIGRATION_STEPS - 1))
}
