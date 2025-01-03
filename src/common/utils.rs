mod checksum;
pub use checksum::*;
pub mod tokens;
pub mod oauth;
use prost::Message as _;

use crate::{app::constant::CURSOR_API2_GET_USER_INFO, chat::aiserver::v1::GetUserInfoResponse};

use super::models::usage::UserUsageInfo;

pub fn parse_bool_from_env(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| match v.to_lowercase().as_str() {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => default,
        })
        .unwrap_or(default)
}

pub fn parse_string_from_env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

pub fn i32_to_u32(value: i32) -> u32 {
    if value < 0 {
        0
    } else {
        value as u32
    }
}

pub async fn get_user_usage(auth_token: &str, checksum: &str) -> Option<UserUsageInfo> {
    // 构建请求客户端
    let client = super::client::build_client(auth_token, checksum, CURSOR_API2_GET_USER_INFO);
    let response = client
        .body(Vec::new())
        .send()
        .await
        .ok()?
        .bytes()
        .await
        .ok()?;
    let user_info = GetUserInfoResponse::decode(response.as_ref()).ok()?;

    user_info.usage.map(|user_usage| UserUsageInfo {
        fast_requests: i32_to_u32(user_usage.gpt4_requests),
        max_fast_requests: i32_to_u32(user_usage.gpt4_max_requests),
    })
}
