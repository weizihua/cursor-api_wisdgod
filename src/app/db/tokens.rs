use crate::app::model::{TokenInfo, TokenStatus};
use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension as _, Result};
use super::Database;
// 限制字段长度
const MAX_TOKEN_LENGTH: usize = 1000; // 限制 token 长度为 1000 字符
const MAX_CHECKSUM_LENGTH: usize = 200; // 限制 checksum 长度为 200 字符
const MAX_ALIAS_LENGTH: usize = 100; // 限制 alias 长度为 100 字符
const MAX_QUERY_LIMIT: usize = 1000; // 最大查询数量限制
pub fn insert_token(token_info: &TokenInfo) -> Result<i64> {
    super::with_db_mut(|conn| Database::insert_token(conn, token_info))
}
pub fn get_tokens_by_user_id(user_id: Option<i64>) -> Result<Vec<TokenInfo>> {
    super::with_db(|conn| Database::get_tokens_by_user_id(conn, user_id))
}
pub fn get_available_tokens_by_user_id(user_id: Option<i64>) -> Result<Vec<TokenInfo>> {
    super::with_db(|conn| Database::get_available_tokens_by_user_id(conn, user_id))
}
pub fn get_token_by_id(id: i64) -> Result<Option<TokenInfo>> {
    super::with_db(|conn| Database::get_token_by_id(conn, id))
}
pub fn get_token_by_token(token: &str) -> Result<Option<TokenInfo>> {
    super::with_db(|conn| Database::get_token_by_token(conn, token))
}
pub fn update_token_status(id: i64, status: TokenStatus) -> Result<()> {
    super::with_db_mut(|conn| Database::update_token_status(conn, id, status))
}
pub fn delete_expired_tokens() -> Result<()> {
    super::with_db_mut(|conn| Database::delete_expired_tokens(conn))
}
pub fn get_token_by_alias_and_user(
    alias: &str,
    current_user_id: i64,
    target_user_id: Option<i64>,
) -> Result<Option<TokenInfo>> {
    super::with_db(|conn| {
        Database::get_token_by_alias_and_user(conn, alias, current_user_id, target_user_id)
    })
}
pub fn update_token(token_info: &TokenInfo) -> Result<()> {
    super::with_db_mut(|conn| Database::update_token(conn, token_info))
}
impl Database {
    pub fn init_tokens_table(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tokens (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            create_at TEXT NOT NULL,
            token TEXT NOT NULL UNIQUE,
            checksum TEXT NOT NULL,
            alias TEXT,
            status INTEGER NOT NULL,
            pengding_at TEXT NOT NULL,
            user_id INTEGER NOT NULL,
            is_public BOOLEAN NOT NULL DEFAULT 0,
            usage TEXT,
            FOREIGN KEY(user_id) REFERENCES users(id),
            UNIQUE(alias, user_id)
        )",
            [],
        )?;
        Ok(())
    }
    pub fn insert_token(conn: &mut Connection, token_info: &TokenInfo) -> Result<i64> {
        // 输入验证
        if token_info.token.len() > MAX_TOKEN_LENGTH {
            return Err(rusqlite::Error::InvalidParameterName(
                "Token too long".to_string(),
            ));
        }
        if token_info.checksum.len() > MAX_CHECKSUM_LENGTH {
            return Err(rusqlite::Error::InvalidParameterName(
                "Checksum too long".to_string(),
            ));
        }
        if let Some(alias) = &token_info.alias {
            if alias.len() > MAX_ALIAS_LENGTH {
                return Err(rusqlite::Error::InvalidParameterName(
                    "Alias too long".to_string(),
                ));
            }
        }
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO tokens (
            create_at, token, checksum, alias,
            status, pengding_at, user_id, is_public, usage
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                Local::now(),
                token_info.token,
                token_info.checksum,
                token_info.alias,
                token_info.status,
                token_info.user_id,
                token_info.is_public,
                token_info.usage,
            ],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(id)
    }
    pub fn get_tokens_by_user_id(
        conn: &Connection,
        user_id: Option<i64>,
    ) -> Result<Vec<TokenInfo>> {
        let mut stmt = conn.prepare(
            "SELECT id, create_at, token, checksum, alias,
                status, pengding_at, user_id, is_public, usage
         FROM tokens
         WHERE user_id IS ?1
         LIMIT ?2",
        )?;
        let tokens_iter = stmt.query_map(
            params![user_id, MAX_QUERY_LIMIT as i64],
            Self::row_to_token_info,
        )?;
        let mut tokens = Vec::with_capacity(100);
        for token in tokens_iter {
            tokens.push(token?);
        }
        Ok(tokens)
    }
    pub fn get_available_tokens_by_user_id(
        conn: &Connection,
        user_id: Option<i64>,
    ) -> Result<Vec<TokenInfo>> {
        let mut stmt = conn.prepare(
            "SELECT id, create_at, token, checksum, alias,
                status, pengding_at, user_id, is_public, usage
         FROM tokens
         WHERE status = 1 AND datetime('now') >= pengding_at AND user_id IS ?1
         LIMIT ?2",
        )?;
        let tokens_iter = stmt.query_map(
            params![user_id, MAX_QUERY_LIMIT as i64],
            Self::row_to_token_info,
        )?;
        let mut tokens = Vec::with_capacity(100);
        for token in tokens_iter {
            tokens.push(token?);
        }
        Ok(tokens)
    }
    pub fn get_token_by_id(conn: &Connection, id: i64) -> Result<Option<TokenInfo>> {
        conn.query_row(
            "SELECT id, create_at, token, checksum, alias,
                status, pengding_at, user_id, is_public, usage
         FROM tokens
         WHERE id = ?1",
            params![id],
            Self::row_to_token_info,
        )
        .optional()
    }
    pub fn get_token_by_token(conn: &Connection, token: &str) -> Result<Option<TokenInfo>> {
        // 输入验证
        if token.len() > MAX_TOKEN_LENGTH {
            return Err(rusqlite::Error::InvalidParameterName(
                "Token too long".to_string(),
            ));
        }
        conn.query_row(
            "SELECT id, create_at, token, checksum, alias,
                status, pengding_at, user_id, is_public, usage
         FROM tokens
         WHERE token = ?1",
            params![token],
            Self::row_to_token_info,
        )
        .optional()
    }
    pub fn update_token_status(conn: &mut Connection, id: i64, status: TokenStatus) -> Result<()> {
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE tokens SET status = ?1 WHERE id = ?2",
            params![status, id],
        )?;
        if status == TokenStatus::Pending {
            tx.execute(
                "UPDATE tokens SET pengding_at = ?1 WHERE id = ?2",
                params![Local::now() + chrono::Duration::minutes(1), id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn delete_expired_tokens(conn: &mut Connection) -> Result<()> {
        // 开始事务
        let tx = conn.transaction()?;
        // 删除过期token相关的日志
        tx.execute(
            "DELETE FROM logs WHERE token_id IN (
            SELECT id FROM tokens
            WHERE status = ?1
        )",
            params![TokenStatus::Expired],
        )?;
        // 删除过期的token
        tx.execute(
            "DELETE FROM tokens WHERE status = ?1",
            params![TokenStatus::Expired],
        )?;
        // 提交事务
        tx.commit()?;
        Ok(())
    }
    pub fn get_token_by_alias_and_user(
        conn: &Connection,
        alias: &str,
        current_user_id: i64,
        target_user_id: Option<i64>,
    ) -> Result<Option<TokenInfo>> {
        // 管理员可以查看所有token
        let is_admin = current_user_id == 0;
        let sql = if is_admin {
            // 管理员查询：如果指定了user_id就查指定用户的，否则查所有
            if target_user_id.is_some() {
                "SELECT id, create_at, token, checksum, alias,
                        status, user_id, is_public, usage
                 FROM tokens
                 WHERE alias = ?1
                 AND status IN (?2, ?3)
                 AND user_id = ?4"
            } else {
                "SELECT id, create_at, token, checksum, alias,
                        status, pengding_at, user_id, is_public, usage
                 FROM tokens
                 WHERE alias = ?1
                 AND status IN (?2, ?3)"
            }
        } else {
            // 普通用户查询：只能查看自己的token
            "SELECT id, create_at, token, checksum, alias,
                    status, pengding_at, user_id, is_public, usage
             FROM tokens
             WHERE alias = ?1
             AND status IN (?2, ?3)
             AND user_id = ?4"
        };
        let target_id = target_user_id.map(|id| id);
        let params: Vec<&dyn rusqlite::ToSql> = if is_admin {
            if let Some(ref id) = target_id {
                vec![&alias, &TokenStatus::Active, &TokenStatus::Pending, id]
            } else {
                vec![&alias, &TokenStatus::Active, &TokenStatus::Pending]
            }
        } else {
            vec![
                &alias,
                &TokenStatus::Active,
                &TokenStatus::Pending,
                &current_user_id,
            ]
        };
        conn.query_row(sql, params.as_slice(), Self::row_to_token_info)
            .optional()
    }
    pub fn update_token(conn: &mut Connection, token_info: &TokenInfo) -> Result<()> {
        // 输入验证
        if token_info.checksum.len() > MAX_CHECKSUM_LENGTH {
            return Err(rusqlite::Error::InvalidParameterName(
                "Checksum too long".to_string(),
            ));
        }
        if let Some(alias) = &token_info.alias {
            if alias.len() > MAX_ALIAS_LENGTH {
                return Err(rusqlite::Error::InvalidParameterName(
                    "Alias too long".to_string(),
                ));
            }
        }
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE tokens SET
                checksum = ?1,
                alias = ?2,
                is_public = ?3
             WHERE id = ?4",
            params![
                token_info.checksum,
                token_info.alias,
                token_info.is_public,
                token_info.id,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }
    fn row_to_token_info(row: &rusqlite::Row<'_>) -> Result<TokenInfo> {
        Ok(TokenInfo {
            id: row.get(0)?,
            create_at: row.get(1)?,
            token: row.get(2)?,
            checksum: row.get(3)?,
            alias: row.get(4)?,
            status: row.get(5)?,
            pengding_at: row.get(6)?,
            user_id: row.get(7)?,
            is_public: row.get(8)?,
            usage: row.get(9)?,
        })
    }
}
