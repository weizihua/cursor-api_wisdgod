use super::Database;
use crate::{app::model::{LogInfo, LogStatus, TokenInfo}, common::models::usage::UserUsageInfo};
use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension as _, Result};
const MAX_PROMPT_LENGTH: usize = 100000; // 限制 prompt 长度为 100000 字符
const MAX_MODEL_LENGTH: usize = 100; // 限制 model 名称长度为 100 字符
const MAX_ERROR_LENGTH: usize = 1000; // 限制 error 信息长度为 1000 字符
const MAX_QUERY_LIMIT: usize = 1000; // 最大查询数量限制
pub fn insert_log(log_info: &LogInfo) -> Result<i64> {
    super::with_db_mut(|conn| Database::insert_log(conn, log_info))
}
pub fn get_logs_by_user_id(user_id: Option<i64>) -> Result<Vec<LogInfo>> {
    super::with_db_mut(|conn| Database::get_logs_by_user_id(conn, user_id))
}
pub fn get_logs_by_token_id(token_id: i64) -> Result<Vec<LogInfo>> {
    super::with_db_mut(|conn| Database::get_logs_by_token_id(conn, token_id))
}
pub fn get_log_by_id(id: i64) -> Result<Option<LogInfo>> {
    super::with_db(|conn| Database::get_log_by_id(conn, id))
}
pub fn update_log_status(id: i64, status: LogStatus, error: Option<String>) -> Result<()> {
    super::with_db_mut(|conn| Database::update_log_status(conn, id, status, error))
}
pub fn clean_user_logs(user_id: i64, limit: usize) -> Result<()> {
    super::with_db_mut(|conn| Database::clean_user_logs(conn, user_id, limit))
}
pub fn get_user_logs_count(user_id: i64) -> Result<i64> {
    super::with_db(|conn| Database::get_user_logs_count(conn, user_id))
}
pub fn update_log_usage(log_id: i64, usage: Option<UserUsageInfo>) -> Result<()> {
    super::with_db_mut(|conn| Database::update_log_usage(conn, log_id, usage))
}
pub fn update_log_prompt(log_id: i64, prompt: Option<String>) -> Result<()> {
    super::with_db_mut(|conn| Database::update_log_prompt(conn, log_id, prompt))
}
impl Database {
    pub fn init_logs_table(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            token_id INTEGER NOT NULL,
            prompt TEXT,
            model TEXT NOT NULL,
            stream BOOLEAN NOT NULL,
            status INTEGER NOT NULL,
            error TEXT,
            FOREIGN KEY(token_id) REFERENCES tokens(id)
        )",
            [],
        )?;
        Ok(())
    }
    pub fn insert_log(conn: &mut Connection, log_info: &LogInfo) -> Result<i64> {
        // 输入验证
        if let Some(prompt) = &log_info.prompt {
            if prompt.len() > MAX_PROMPT_LENGTH {
                return Err(rusqlite::Error::InvalidParameterName(
                    "Prompt too long".to_string(),
                ));
            }
        }
        if log_info.model.len() > MAX_MODEL_LENGTH {
            return Err(rusqlite::Error::InvalidParameterName(
                "Model name too long".to_string(),
            ));
        }
        if let Some(error) = &log_info.error {
            if error.len() > MAX_ERROR_LENGTH {
                return Err(rusqlite::Error::InvalidParameterName(
                    "Error message too long".to_string(),
                ));
            }
        }
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO logs (
            timestamp, token_id, prompt, model,
            stream, status, error
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                Local::now(),
                log_info.token_info.id,
                log_info.prompt,
                log_info.model,
                log_info.stream,
                log_info.status,
                log_info.error,
            ],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(id)
    }
    pub fn get_logs_by_user_id(
        conn: &mut Connection,
        user_id: Option<i64>,
    ) -> Result<Vec<LogInfo>> {
        let mut stmt = conn.prepare(
            "SELECT l.id, l.timestamp, l.prompt, l.model, l.stream, l.status, l.error,
              t.id, t.create_at, t.token, t.checksum, t.alias,
              t.status, t.pengding_at, t.user_id, t.is_public, t.usage
       FROM logs l
       JOIN tokens t ON l.token_id = t.id
       WHERE t.user_id IS ?1
       ORDER BY l.timestamp DESC
       LIMIT 100",
        )?;
        let logs_iter = stmt.query_map(params![user_id], Self::row_to_log_info)?;
        let mut logs = Vec::with_capacity(100);
        for log in logs_iter {
            logs.push(log?);
        }
        Ok(logs)
    }
    pub fn get_logs_by_token_id(conn: &mut Connection, token_id: i64) -> Result<Vec<LogInfo>> {
        // 使用事务确保一致性
        let tx = conn.transaction()?;
        // 先获取token信息
        let token = Self::get_token_by_id(&tx, token_id)?
            .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;
        // 查询日志记录
        let logs = {
            let mut stmt = tx.prepare(
                "SELECT id, timestamp, prompt, model, stream, status, error
             FROM logs
             WHERE token_id = ?1
             ORDER BY timestamp DESC
             LIMIT ?2",
            )?;
            let mut logs = Vec::with_capacity(100);
            let logs_iter = stmt.query_map(params![token_id, MAX_QUERY_LIMIT as i64], |row| {
                Ok(LogInfo {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    prompt: row.get(2)?,
                    model: row.get(3)?,
                    stream: row.get(4)?,
                    status: row.get(5)?,
                    error: row.get(6)?,
                    token_info: token.clone(),
                })
            })?;
            for log in logs_iter {
                logs.push(log?);
            }
            logs
        };
        tx.commit()?;
        Ok(logs)
    }
    pub fn get_log_by_id(conn: &Connection, id: i64) -> Result<Option<LogInfo>> {
        conn.query_row(
            "SELECT l.id, l.timestamp, l.prompt, l.model, l.stream, l.status, l.error,
                t.id, t.create_at, t.token, t.checksum, t.alias,
                t.status, t.pengding_at, t.user_id, t.is_public, t.usage
         FROM logs l
         JOIN tokens t ON l.token_id = t.id
         WHERE l.id = ?1",
            params![id],
            Self::row_to_log_info,
        )
        .optional()
    }
    pub fn update_log_status(
        conn: &mut Connection,
        id: i64,
        status: LogStatus,
        error: Option<String>,
    ) -> Result<()> {
        // 验证 error 长度
        if let Some(error_msg) = &error {
            if error_msg.len() > MAX_ERROR_LENGTH {
                return Err(rusqlite::Error::InvalidParameterName(
                    "Error message too long".to_string(),
                ));
            }
        }
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE logs SET status = ?1, error = ?2 WHERE id = ?3",
            params![status, error, id],
        )?;
        tx.commit()?;
        Ok(())
    }
    fn row_to_log_info(row: &rusqlite::Row<'_>) -> Result<LogInfo> {
        let token_info = TokenInfo {
            id: row.get(7)?,
            create_at: row.get(8)?,
            token: row.get(9)?,
            checksum: row.get(10)?,
            alias: row.get(11)?,
            status: row.get(12)?,
            pengding_at: row.get(13)?,
            user_id: row.get(14)?,
            is_public: row.get(15)?,
            usage: row.get(16)?,
        };
        Ok(LogInfo {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            prompt: row.get(2)?,
            model: row.get(3)?,
            stream: row.get(4)?,
            status: row.get(5)?,
            error: row.get(6)?,
            token_info,
        })
    }
    pub fn clean_user_logs(conn: &mut Connection, user_id: i64, limit: usize) -> Result<()> {
        let tx = conn.transaction()?;
        // 先获取所有需要删除的日志ID
        let mut stmt = tx.prepare(
            "WITH RankedLogs AS (
                SELECT l.id,
                       ROW_NUMBER() OVER (ORDER BY l.timestamp DESC) as rn
                FROM logs l
                JOIN tokens t ON l.token_id = t.id
                WHERE t.user_id = ?1
            )
            SELECT id FROM RankedLogs WHERE rn > ?2",
        )?;
        let log_ids: Vec<i64> = stmt
            .query_map(params![user_id, limit as i64], |row| row.get(0))?
            .collect::<Result<Vec<_>>>()?;
        // 确保 stmt 被释放
        drop(stmt);
        // 如果有需要删除的日志
        if !log_ids.is_empty() {
            // 直接更新状态，不使用 IN 子句
            tx.execute(
                "UPDATE logs SET status = ?1
                 WHERE id IN (
                     SELECT l.id
                     FROM logs l
                     JOIN tokens t ON l.token_id = t.id
                     WHERE t.user_id = ?2
                     ORDER BY l.timestamp ASC
                     LIMIT -1
                     OFFSET ?3
                 )",
                params![LogStatus::Deleted as u8, user_id, limit],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn get_user_logs_count(conn: &Connection, user_id: i64) -> Result<i64> {
        conn.query_row(
            "SELECT COUNT(*)
             FROM logs l
             JOIN tokens t ON l.token_id = t.id
             WHERE t.user_id = ?1 AND l.status != ?2",
            params![user_id, LogStatus::Deleted],
            |row| row.get(0),
        )
    }
    pub fn update_log_usage(conn: &mut Connection, log_id: i64, usage: Option<UserUsageInfo>) -> Result<()> {
        let tx = conn.transaction()?;
        tx.execute("UPDATE logs SET usage = ?1 WHERE id = ?2", params![usage, log_id])?;
        tx.commit()?;
        Ok(())
    }
    pub fn update_log_prompt(conn: &mut Connection, log_id: i64, prompt: Option<String>) -> Result<()> {
        let tx = conn.transaction()?;
        tx.execute("UPDATE logs SET prompt = ?1 WHERE id = ?2", params![prompt, log_id])?;
        tx.commit()?;
        Ok(())
    }
}
