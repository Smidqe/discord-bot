use super::Result;
use crate::CONFIG;
use rusqlite::{named_params, types::Value, Connection, OptionalExtension};
use serenity::model::prelude::*;
use std::rc::Rc;

pub fn user_online(conn: &Connection, user: &User) -> Result<()> {
    conn.prepare_cached(
        "
        INSERT INTO users (id)
        VALUES (:id)
        ON CONFLICT (id)
        DO UPDATE SET
            first_online = COALESCE(first_online, datetime('now')),
            last_online = datetime('now')
        ",
    )?
    .execute_named(named_params! {
        ":id": user.id.to_string(),
    })?;

    conn.prepare_cached(
        "
        INSERT INTO usernames (id, name, discriminator)
        VALUES (:id, :name, :discriminator)
        ON CONFLICT (id, name, discriminator)
        DO UPDATE SET
            first_online = COALESCE(first_online, datetime('now')),
            last_online = datetime('now')
        ",
    )?
    .execute_named(named_params! {
        ":id": user.id.to_string(),
        ":name": user.name,
        ":discriminator": format!("{:04}", user.discriminator),
    })?;

    Ok(())
}

pub fn member_online(conn: &Connection, member: &Member) -> Result<()> {
    let user = member.user.read();
    user_online(conn, &user)?;

    if let Some(ref nick) = member.nick {
        conn.prepare_cached(
            "
            INSERT INTO nicks (id, nick)
            VALUES (:id, :nick)
            ON CONFLICT (id, nick)
            DO UPDATE SET
                first_online = COALESCE(first_online, datetime('now')),
                last_online = datetime('now')
            ",
        )?
        .execute_named(named_params! {
            ":id": user.id.to_string(),
            ":nick": nick,
        })?;
    }

    Ok(())
}

pub fn user_offline(conn: &Connection, user: &User) -> Result<()> {
    conn.prepare_cached(
        "
        INSERT OR IGNORE INTO users (id, first_online, last_online)
        VALUES (:id, NULL, NULL)
        ",
    )?
    .execute_named(named_params! {
        ":id": user.id.to_string(),
    })?;

    conn.prepare_cached(
        "
        INSERT OR IGNORE INTO usernames (id, name, discriminator, first_online, last_online)
        VALUES (:id, :name, :discriminator, NULL, NULL)
        ",
    )?
    .execute_named(named_params! {
        ":id": user.id.to_string(),
        ":name": user.name,
        ":discriminator": format!("{:04}", user.discriminator),
    })?;

    Ok(())
}

pub fn member_offline(conn: &Connection, member: &Member) -> Result<()> {
    let user = member.user.read();
    user_offline(conn, &user)?;

    if let Some(ref nick) = member.nick {
        conn.prepare_cached(
            "
            INSERT OR IGNORE INTO nicks (id, nick, first_online, last_online)
            VALUES (:id, :nick, NULL, NULL)
            ",
        )?
        .execute_named(named_params! {
            ":id": user.id.to_string(),
            ":nick": nick,
        })?;
    }

    Ok(())
}

pub fn user_message(conn: &Connection, user: UserId) -> Result<()> {
    conn.prepare_cached(
        "
        UPDATE users SET
            first_message = COALESCE(first_message, datetime('now')),
            last_message = datetime('now')
        WHERE id = :id
        ",
    )?
    .execute_named(named_params! {
        ":id": user.to_string(),
    })?;

    Ok(())
}

pub fn cache_message(conn: &Connection, message: &Message) -> Result<()> {
    conn.prepare_cached(
        "
        INSERT OR REPLACE INTO messages (id, user_id, json)
        VALUES (:id, :user_id, :json)
        ",
    )?
    .execute_named(named_params! {
        ":id": message.id.to_string(),
        ":user_id": message.author.id.to_string(),
        ":json": serde_json::to_string(&message)?,
    })?;

    conn.prepare_cached(
        "
        DELETE FROM messages
        WHERE id NOT IN (
            SELECT id FROM messages
            ORDER BY time DESC
            LIMIT :history
        )
        ",
    )?
    .execute_named(named_params! {
        ":history": CONFIG.discord.deleted_msg_cache,
    })?;

    Ok(())
}

pub fn get_message(conn: &Connection, id: MessageId) -> Result<Option<Message>> {
    if let Some(json) = conn
        .prepare_cached(
            "
            SELECT json FROM messages
            WHERE id = :id
            ",
        )?
        .query_row_named(
            named_params! {
                ":id": id.to_string(),
            },
            |row| row.get::<_, String>(0),
        )
        .optional()?
    {
        Ok(serde_json::from_str(&json)?)
    } else {
        Ok(None)
    }
}

pub fn set_sticky_roles(
    conn: &Connection,
    user: UserId,
    roles: impl IntoIterator<Item = RoleId>,
) -> Result<()> {
    let ids = Rc::new(
        roles
            .into_iter()
            .map(|id| Value::from(id.to_string()))
            .collect(),
    );

    conn.prepare_cached(
        "
        DELETE FROM sticky_roles
        WHERE user_id = :user_id AND role_id NOT IN (SELECT value FROM rarray(:role_ids))
        ",
    )?
    .execute_named(named_params! {
        ":user_id": user.to_string(),
        ":role_ids": &ids,
    })?;

    conn.prepare_cached(
        "
        INSERT OR IGNORE INTO sticky_roles (user_id, role_id)
        SELECT :user_id, value FROM rarray(:role_ids)
        ",
    )?
    .execute_named(named_params! {
        ":user_id": user.to_string(),
        ":role_ids": &ids,
    })?;

    Ok(())
}

pub fn get_sticky_roles(conn: &Connection, user: UserId) -> Result<Vec<RoleId>> {
    let ids: rusqlite::Result<Vec<String>> = conn
        .prepare_cached(
            "
            SELECT role_id FROM sticky_roles
            WHERE user_id = :user_id
            ",
        )?
        .query_map_named(
            named_params! {
                ":user_id": user.to_string(),
            },
            |row| row.get(0),
        )?
        .collect();

    Ok(ids?
        .into_iter()
        .filter_map(|id| id.parse().ok().map(RoleId))
        .collect())
}

pub fn gib_seen(conn: &Connection, id: u32) -> Result<()> {
    conn.prepare_cached(
        "
        INSERT OR REPLACE INTO gib_seen (id)
        VALUES (:id)
        ",
    )?
    .execute_named(named_params! {
        ":id": id,
    })?;

    conn.prepare_cached(
        "
        DELETE FROM gib_seen
        WHERE id NOT IN (
            SELECT id FROM gib_seen
            ORDER BY time DESC
            LIMIT :history
        )
        ",
    )?
    .execute_named(named_params! {
        ":history": CONFIG.gib.history,
    })?;

    Ok(())
}

pub fn gib_is_seen(conn: &Connection, id: u32) -> Result<bool> {
    Ok(conn
        .prepare_cached(
            "
            SELECT id FROM gib_seen
            WHERE id = :id
            LIMIT 1
            ",
        )?
        .query_named(named_params! {
            ":id": id,
        })?
        .next()?
        .is_some())
}

pub fn reddit_seen(conn: &Connection, ids: impl IntoIterator<Item = String>) -> Result<()> {
    conn.prepare_cached(
        "
        INSERT OR IGNORE INTO reddit_seen (id)
        SELECT value FROM rarray(:ids)
        ",
    )?
    .execute_named(named_params! {
        ":ids": Rc::new(ids.into_iter().map(Value::from).collect()),
    })?;

    Ok(())
}

pub fn reddit_contains_unseen(
    conn: &Connection,
    ids: impl IntoIterator<Item = String>,
) -> Result<bool> {
    Ok(conn
        .prepare_cached(
            "
            SELECT value FROM rarray(:ids)
            WHERE value NOT IN (SELECT id FROM reddit_seen)
            LIMIT 1
            ",
        )?
        .query_named(named_params! {
            ":ids": Rc::new(ids.into_iter().map(Value::from).collect()),
        })?
        .next()?
        .is_some())
}
