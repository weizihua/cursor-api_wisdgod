mod logs;
mod tokens;
mod users;

use chrono::Utc;
use rusqlite::{Connection, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use tokio::time::{self, Duration};

pub struct Database {
    conn: Connection,
}

// 全局静态 Database 实例
static DB: OnceLock<Mutex<Database>> = OnceLock::new();

// 用于控制清理任务的标志
static CLEANER_RUNNING: AtomicBool = AtomicBool::new(false);

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;

        // 启用 WAL 模式
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;          -- 启用 WAL 模式
            PRAGMA synchronous = NORMAL;        -- 适度的同步模式
            PRAGMA cache_size = -64000;         -- 64MB 缓存
            PRAGMA foreign_keys = ON;           -- 启用外键约束
            PRAGMA temp_store = MEMORY;         -- 临时表使用内存
            PRAGMA mmap_size = 30000000000;     -- 30GB mmap
        ",
        )?;

        // 按照依赖顺序初始化表
        Self::init_users_table(&conn)?;
        Self::init_tokens_table(&conn)?;
        Self::init_logs_table(&conn)?;

        Ok(Self { conn })
    }

    pub fn init(path: &str) -> Result<()> {
        let db = Database::new(path)?;
        DB.set(Mutex::new(db)).map_err(|_| {
            rusqlite::Error::InvalidParameterName("Database already initialized".into())
        })
    }

    pub fn global() -> &'static Mutex<Database> {
        DB.get().expect("Database not initialized")
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    // 启动定时清理任务
    pub fn start_cleaner() {
        // 确保只启动一次
        if CLEANER_RUNNING.swap(true, Ordering::SeqCst) {
            return;
        }

        tokio::spawn(async move {
            loop {
                // 等待到下一个 UTC 20:00
                let now = Utc::now();
                let next = (now.date_naive() + chrono::Duration::days(1))
                    .and_hms_opt(20, 0, 0)
                    .unwrap();
                let duration = next.signed_duration_since(now.naive_utc());

                time::sleep(Duration::from_secs(duration.num_seconds() as u64)).await;

                if let Err(e) = Self::clean_expired_tokens().await {
                    eprintln!("Failed to clean expired tokens: {}", e);
                }
            }
        });
    }

    // 清理过期数据
    async fn clean_expired_tokens() -> Result<()> {
        with_db_mut(|conn| {
            let tx = conn.transaction()?;

            // 删除过期token相关的日志
            tx.execute(
                "DELETE FROM logs WHERE token_id IN (
                    SELECT id FROM tokens 
                    WHERE status = 2 OR 
                          (status = 1 AND duration > 0 AND 
                           datetime(create_at, '+' || (duration / 86400) || ' days') < datetime('now'))
                )",
                [],
            )?;

            // 删除过期token
            tx.execute(
                "DELETE FROM tokens 
                 WHERE status = 2 OR 
                       (status = 1 AND duration > 0 AND 
                        datetime(create_at, '+' || (duration / 86400) || ' days') < datetime('now'))",
                [],
            )?;

            // 执行WAL清理
            tx.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;

            tx.commit()
        })
    }

    // 停止清理任务
    // pub fn stop_cleaner() {
    //     CLEANER_RUNNING.store(false, Ordering::SeqCst);
    // }
}

pub fn with_db<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&Connection) -> Result<T>,
{
    let guard = Database::global().lock().expect("Database lock poisoned");
    f(guard.conn())
}

pub fn with_db_mut<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&mut Connection) -> Result<T>,
{
    let mut guard = Database::global().lock().expect("Database lock poisoned");
    f(guard.conn_mut())
}

// 重新导出子模块
pub use self::logs::*;
pub use self::tokens::*;
pub use self::users::*;

/*
// 以下是可选的扩展功能,暂时注释掉

impl Drop for Database {
    fn drop(&mut self) {
        // 这里可以添加清理代码
        // Connection 会自动关闭，但如果有其他清理工作可以在这里进行
    }
}

use std::sync::LazyLock;

static DB_CONFIG: LazyLock<DbConfig> = LazyLock::new(|| {
    DbConfig {
        max_connections: 10,
        timeout: std::time::Duration::from_secs(30),
    }
});

struct DbConfig {
    max_connections: u32,
    timeout: std::time::Duration,
}

pub fn example_usage() -> Result<()> {
    Database::init("path/to/db.sqlite")?;
    println!("Max connections: {}", DB_CONFIG.max_connections);
    with_db(|conn| {
        Ok(())
    })?;
    with_db_mut(|conn| {
        let tx = conn.transaction()?;
        tx.commit()
    })
}
*/

// 在应用启动时初始化
pub async fn init_database(path: &str) -> Result<()> {
    Database::init(path)?;
    Database::start_cleaner();
    Ok(())
}
