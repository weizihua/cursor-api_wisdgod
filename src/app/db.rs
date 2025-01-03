use crate::app::model::{RequestLog, TokenInfo};
use crate::common::models::usage::UserUsageInfo;
use chrono::{DateTime, Local};
use lazy_static::lazy_static;
use rusqlite::params;
use rusqlite::{Connection, Result};
use std::path::Path;
use std::sync::Mutex;

const DB_PATH: &str = "logs/sqlite.db";

pub struct AppDb {
    conn: Connection,
}

impl AppDb {
    pub fn new() -> Result<Self> {
        // 确保目录存在
        if let Some(parent) = Path::new(DB_PATH).parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
                    Some(e.to_string()),
                )
            })?;
        }

        let conn = Connection::open(DB_PATH)?;

        // 启用WAL模式以提升性能
        conn.execute_batch("PRAGMA journal_mode = WAL")?;

        // 创建token信息表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS token_infos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                token TEXT NOT NULL UNIQUE,
                checksum TEXT NOT NULL,
                alias TEXT,
                fast_requests INTEGER,
                max_fast_requests INTEGER
            )",
            [],
        )?;

        // 创建请求日志表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS request_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                model TEXT NOT NULL,
                token_id INTEGER NOT NULL,
                prompt TEXT,
                stream BOOLEAN NOT NULL,
                status TEXT NOT NULL,
                error TEXT,
                FOREIGN KEY(token_id) REFERENCES token_infos(id)
            )",
            [],
        )?;

        // 创建索引
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_token ON token_infos(token)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_timestamp_model ON request_logs(timestamp, model)",
            [],
        )?;

        Ok(Self { conn })
    }

    fn get_or_create_token_info(&self, token_info: &TokenInfo) -> Result<i64> {
        let mut stmt = self.conn.prepare_cached(
            "INSERT OR REPLACE INTO token_infos (token, checksum, alias, fast_requests, max_fast_requests)
             VALUES (?1, ?2, ?3, ?4, ?5)
             RETURNING id"
        )?;

        stmt.query_row(
            params![
                &token_info.token,
                &token_info.checksum,
                &token_info.alias,
                token_info.usage.as_ref().map(|u| u.fast_requests),
                token_info.usage.as_ref().map(|u| u.max_fast_requests),
            ],
            |row| row.get(0),
        )
    }

    pub fn add_log(&self, log: &RequestLog) -> Result<()> {
        let token_id = self.get_or_create_token_info(&log.token_info)?;

        self.conn.execute(
            "INSERT INTO request_logs (timestamp, model, token_id, prompt, stream, status, error) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                log.timestamp.to_rfc3339(),
                &log.model,
                token_id,
                &log.prompt,
                log.stream,
                &log.status,
                &log.error,
            ],
        )?;
        Ok(())
    }

    fn map_row_to_log(&self, row: &rusqlite::Row) -> Result<RequestLog> {
        let token_id: i64 = row.get(3)?;
        let token_info = self.get_token_info_by_id(token_id)?;

        Ok(RequestLog {
            id: row.get(0)?,
            timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                .unwrap()
                .with_timezone(&Local),
            model: row.get(2)?,
            token_info,
            prompt: row.get(4)?,
            stream: row.get(5)?,
            status: row.get(6)?,
            error: row.get(7)?,
        })
    }

    fn get_token_info_by_id(&self, id: i64) -> Result<TokenInfo> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT token, checksum, alias, fast_requests, max_fast_requests 
             FROM token_infos 
             WHERE id = ?",
        )?;

        stmt.query_row([id], |row| {
            Ok(TokenInfo {
                token: row.get(0)?,
                checksum: row.get(1)?,
                alias: row.get(2)?,
                usage: Some(UserUsageInfo {
                    fast_requests: row.get(3)?,
                    max_fast_requests: row.get(4)?,
                }),
            })
        })
    }

    pub fn get_token_infos(&self) -> Result<Vec<TokenInfo>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT token, checksum, alias, fast_requests, max_fast_requests 
             FROM token_infos",
        )?;

        let tokens = stmt.query_map([], |row| {
            Ok(TokenInfo {
                token: row.get(0)?,
                checksum: row.get(1)?,
                alias: row.get(2)?,
                usage: Some(UserUsageInfo {
                    fast_requests: row.get(3)?,
                    max_fast_requests: row.get(4)?,
                }),
            })
        })?;
        tokens.collect()
    }

    pub fn get_recent_logs(&self, limit: i64) -> Result<Vec<RequestLog>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT r.id, r.timestamp, r.model, r.token_id, r.prompt, r.stream, r.status, r.error, t.token, t.checksum, t.alias, t.fast_requests, t.max_fast_requests
             FROM request_logs r
             JOIN token_infos t ON r.token_id = t.id
             ORDER BY r.timestamp DESC 
             LIMIT ?",
        )?;

        let logs = stmt.query_map([limit], |row| {
            Ok(RequestLog {
                id: row.get(0)?,
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                    .unwrap()
                    .with_timezone(&Local),
                model: row.get(2)?,
                token_info: TokenInfo {
                    token: row.get(8)?,
                    checksum: row.get(9)?,
                    alias: row.get(10)?,
                    usage: Some(UserUsageInfo {
                        fast_requests: row.get(11)?,
                        max_fast_requests: row.get(12)?,
                    }),
                },
                prompt: row.get(4)?,
                stream: row.get(5)?,
                status: row.get(6)?,
                error: row.get(7)?,
            })
        })?;
        logs.collect()
    }

    pub fn get_logs_by_timerange(
        &self,
        start: DateTime<Local>,
        end: DateTime<Local>,
    ) -> Result<Vec<RequestLog>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT r.id, r.timestamp, r.model, r.token_id, r.prompt, r.stream, r.status, r.error, t.token, t.checksum, t.alias, t.fast_requests, t.max_fast_requests
             FROM request_logs r
             JOIN token_infos t ON r.token_id = t.id
             WHERE r.timestamp BETWEEN ?1 AND ?2 
             ORDER BY r.timestamp DESC",
        )?;

        let logs = stmt.query_map([start.to_rfc3339(), end.to_rfc3339()], |row| {
            Ok(RequestLog {
                id: row.get(0)?,
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                    .unwrap()
                    .with_timezone(&Local),
                model: row.get(2)?,
                token_info: TokenInfo {
                    token: row.get(8)?,
                    checksum: row.get(9)?,
                    alias: row.get(10)?,
                    usage: Some(UserUsageInfo {
                        fast_requests: row.get(11)?,
                        max_fast_requests: row.get(12)?,
                    }),
                },
                prompt: row.get(4)?,
                stream: row.get(5)?,
                status: row.get(6)?,
                error: row.get(7)?,
            })
        })?;
        logs.collect()
    }

    pub fn update_token_info(&self, token_info: &TokenInfo) -> Result<()> {
        self.conn.execute(
          "INSERT OR REPLACE INTO token_infos (token, checksum, alias, fast_requests, max_fast_requests)
           VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                &token_info.token,
                &token_info.checksum,
                &token_info.alias,
                token_info.usage.as_ref().map(|u| u.fast_requests),
                token_info.usage.as_ref().map(|u| u.max_fast_requests),
            ],
        )?;
        Ok(())
    }
}

lazy_static! {
    pub static ref APP_DB: Mutex<AppDb> =
        Mutex::new(AppDb::new().expect("Failed to initialize database"));
}
