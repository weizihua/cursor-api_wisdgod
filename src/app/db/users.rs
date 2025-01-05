use crate::app::model::UserInfo;
use crate::common::utils::oauth::ForumUser;
use chrono::{DateTime, Local};
use rusqlite::{params, Connection, OptionalExtension as _, Result};
use crate::app::lazy::ADMIN_AUTH_TOKEN;
use super::Database;
// 限制字段长度
const MAX_USERNAME_LENGTH: usize = 100; // 限制用户名长度为 100 字符
const MAX_NAME_LENGTH: usize = 100; // 限制姓名长度为 100 字符
const MAX_QUERY_LIMIT: usize = 1000; // 最大查询数量限制
pub fn insert_user(user: &ForumUser) -> Result<i64> {
    super::with_db_mut(|conn| Database::insert_user(conn, user))
}
pub fn get_user_by_id(id: i64) -> Result<Option<UserInfo>> {
    super::with_db(|conn| Database::get_user_by_id(conn, id))
}
pub fn get_user_by_forum_id(forum_id: i64) -> Result<Option<UserInfo>> {
    super::with_db(|conn| Database::get_user_by_forum_id(conn, forum_id))
}
pub fn update_user_ban(forum_id: i64, ban_expired_at: Option<DateTime<Local>>, ban_count: u32) -> Result<()> {
    super::with_db_mut(|conn| Database::update_user_ban(conn, forum_id, ban_expired_at, ban_count))
}
pub fn update_user_auth_token(forum_id: i64, auth_token: Option<String>) -> Result<()> {
    super::with_db_mut(|conn| Database::update_user_auth_token(conn, forum_id, auth_token))
}
pub fn get_user_by_auth_token(auth_token: &str) -> Result<Option<UserInfo>> {
    super::with_db(|conn| Database::get_user_by_auth_token(conn, auth_token))
}
impl Database {
    pub fn init_users_table(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            forum_id INTEGER NOT NULL UNIQUE,
            username TEXT NOT NULL,
            name TEXT NOT NULL,
            trust_level INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            ban_expired_at TEXT,
            ban_count INTEGER NOT NULL,
            auth_token TEXT
        )",
            [],
        )?;
        let admin_exists: bool = conn
            .query_row("SELECT EXISTS(SELECT 1 FROM users WHERE id = 0)", [], |row| {
                row.get(0)
            })?;
        if !admin_exists {
            conn.execute(
                "INSERT INTO users (
                    id, forum_id, username, name, trust_level,
                    created_at, ban_expired_at, ban_count, auth_token
                ) VALUES (
                    0, 0, 'admin', 'Administrator', 255,
                    ?1, NULL, 0, ?2
                )",
                params![Local::now(), &*ADMIN_AUTH_TOKEN],
            )?;
        }
        Ok(())
    }
    pub fn insert_user(conn: &mut Connection, user: &ForumUser) -> Result<i64> {
        // 输入验证
        if user.username.len() > MAX_USERNAME_LENGTH {
            return Err(rusqlite::Error::InvalidParameterName(
                "Username too long".to_string(),
            ));
        }
        if user.name.len() > MAX_NAME_LENGTH {
            return Err(rusqlite::Error::InvalidParameterName(
                "Name too long".to_string(),
            ));
        }
        let tx = conn.transaction()?;
        tx.execute(
        "INSERT INTO users (forum_id, username, name, trust_level, created_at, ban_expired_at, ban_count, auth_token)
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            user.id,
            user.username,
            user.name,
            user.trust_level,
            Local::now(),
            Option::<DateTime<Local>>::None,
            0,
            Option::<String>::None
        ],
    )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(id)
    }
    pub fn get_user_by_id(conn: &Connection, id: i64) -> Result<Option<UserInfo>> {
        conn.query_row(
            "SELECT id, forum_id, username, name, trust_level, created_at, ban_expired_at, ban_count, auth_token
       FROM users
       WHERE id = ?1
       LIMIT 1",
            params![id],
            |row| {
                Ok(UserInfo {
                    id: row.get(0)?,
                    forum_id: row.get(1)?,
                    username: row.get(2)?,
                    name: row.get(3)?,
                    trust_level: row.get(4)?,
                    created_at: row.get(5)?,
                    ban_expired_at: row.get(6)?,
                    ban_count: row.get(7)?,
                    auth_token: row.get(8)?,
                })
            },
        )
        .optional()
    }
    pub fn get_user_by_forum_id(conn: &Connection, forum_id: i64) -> Result<Option<UserInfo>> {
        conn.query_row(
            "SELECT id, forum_id, username, name, trust_level, created_at, ban_expired_at, ban_count, auth_token
       FROM users
       WHERE forum_id = ?1
       LIMIT 1",
            params![forum_id],
            |row| {
                Ok(UserInfo {
                    id: row.get(0)?,
                    forum_id: row.get(1)?,
                    username: row.get(2)?,
                    name: row.get(3)?,
                    trust_level: row.get(4)?,
                    created_at: row.get(5)?,
                    ban_expired_at: row.get(6)?,
                    ban_count: row.get(7)?,
                    auth_token: row.get(8)?,
                })
            },
        )
        .optional()
    }
    pub fn update_user_ban(conn: &mut Connection, forum_id: i64, ban_expired_at: Option<DateTime<Local>>, ban_count: u32) -> Result<()> {
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE users SET ban_expired_at = ?1, ban_count = ?2 WHERE forum_id = ?3",
            params![ban_expired_at, ban_count, forum_id],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn update_user_auth_token(
        conn: &mut Connection,
        forum_id: i64,
        auth_token: Option<String>,
    ) -> Result<()> {
        let tx = conn.transaction()?;
        // 检查 auth_token 是否已存在
        if let Some(token) = &auth_token {
            let existing = tx.query_row(
                "SELECT forum_id FROM users WHERE auth_token = ?1 AND forum_id != ?2",
                params![token, forum_id],
                |_| Ok(()),
            );
            if existing.optional()?.is_some() {
                return Err(rusqlite::Error::InvalidParameterName(
                    "Auth token already exists".to_string(),
                ));
            }
        }
        tx.execute(
            "UPDATE users SET auth_token = ?1 WHERE forum_id = ?2",
            params![auth_token, forum_id],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn get_user_by_auth_token(conn: &Connection, auth_token: &str) -> Result<Option<UserInfo>> {
        conn.query_row(
            "SELECT id, forum_id, username, name, trust_level, created_at, ban_expired_at, ban_count, auth_token
         FROM users
         WHERE auth_token = ?1
         LIMIT 1",
            params![auth_token],
            |row| {
                Ok(UserInfo {
                    id: row.get(0)?,
                    forum_id: row.get(1)?,
                    username: row.get(2)?,
                    name: row.get(3)?,
                    trust_level: row.get(4)?,
                    created_at: row.get(5)?,
                    ban_expired_at: row.get(6)?,
                    ban_count: row.get(7)?,
                    auth_token: row.get(8)?,
                })
            },
        )
        .optional()
    }
}
