use crate::{chat::model::Message, common::models::usage::UserUsageInfo};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone)]
pub enum LogStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "deleted")]
    Deleted,
}

impl rusqlite::types::FromSql for LogStatus {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match value.as_i64()? {
            0 => Ok(LogStatus::Pending),
            1 => Ok(LogStatus::Success), 
            2 => Ok(LogStatus::Failed),
            3 => Ok(LogStatus::Deleted),
            _ => Err(rusqlite::types::FromSqlError::OutOfRange(value.as_i64()?)),
        }
    }
}

impl rusqlite::ToSql for LogStatus {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::from(match self {
            LogStatus::Pending => 0u8,
            LogStatus::Success => 1u8,
            LogStatus::Failed => 2u8,
            LogStatus::Deleted => 3u8,
        }))
    }
}

// 请求日志
#[derive(Serialize, Clone)]
pub struct LogInfo {
    #[serde(skip_serializing)]
    pub id: i64,
    pub timestamp: DateTime<Local>,
    #[serde(skip_serializing_if = "TokenInfo::is_hide")]
    pub token_info: TokenInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    pub model: String,
    pub stream: bool,
    pub status: LogStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// 聊天请求
#[derive(Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub stream: bool,
}

#[derive(Serialize, PartialEq, Clone)]
pub enum TokenStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "expired")]
    Expired,
    #[serde(rename = "deleted")]
    Deleted,
}

impl rusqlite::types::FromSql for TokenStatus {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match value.as_i64()? {
            0 => Ok(TokenStatus::Pending),
            1 => Ok(TokenStatus::Active), 
            2 => Ok(TokenStatus::Expired),
            3 => Ok(TokenStatus::Deleted),
            _ => Err(rusqlite::types::FromSqlError::OutOfRange(value.as_i64()?)),
        }
    }
}

impl rusqlite::ToSql for TokenStatus {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::from(match self {
            TokenStatus::Pending => 0u8,
            TokenStatus::Active => 1u8,
            TokenStatus::Expired => 2u8,
            TokenStatus::Deleted => 3u8,
        }))
    }
}

// 用于存储 token 信息
#[derive(Serialize, Clone)]
pub struct TokenInfo {
    #[serde(skip_serializing)]
    pub id: i64,
    pub create_at: DateTime<Local>,
    pub token: String,
    pub checksum: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    pub status: TokenStatus,
    pub pengding_at: DateTime<Local>,
    #[serde(skip_serializing)]
    pub user_id: i64,
    pub is_public: bool, // 公益
    #[serde(skip_serializing_if = "Option::is_none")] 
    pub usage: Option<UserUsageInfo>,
}

impl TokenInfo {
    pub fn is_hide(&self) -> bool {
        self.status == TokenStatus::Deleted
    }
}

// TokenUpdateRequest 结构体
#[derive(Deserialize)]
pub struct TokenUpdateRequest {
    pub token: String,
    pub checksum: String,
    #[serde(default)]
    pub alias: Option<String>,
    #[serde(default)]
    pub is_public: bool,
}

#[derive(Clone)]
pub struct UserInfo {
    pub id: i64,
    pub forum_id: i64,
    pub username: String, // 论坛用户名
    pub name: String, // 论坛昵称
    pub trust_level: u8,
    pub created_at: DateTime<Local>,
    pub ban_expired_at: Option<DateTime<Local>>, // 封禁到期时间
    pub ban_count: u32, // 封禁次数
    pub auth_token: Option<String>,
}
