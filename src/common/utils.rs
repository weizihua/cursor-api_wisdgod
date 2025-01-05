mod checksum;
pub use checksum::*;
mod token;
pub use token::*;
pub mod oauth;
use prost::Message as _;

use crate::{app::constant::{CURSOR_API2_GET_USER_INFO, TRUE, FALSE}, chat::aiserver::v1::GetUserInfoResponse};

use super::models::usage::{StripeProfile, UserUsageInfo};

pub fn parse_bool_from_env(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| match v.to_lowercase().as_str() {
            TRUE | "1" => true,
            FALSE | "0" => false,
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

    let (mtype, trial_days) = get_stripe_profile(auth_token).await.unwrap_or_default();

    user_info.usage.map(|user_usage| UserUsageInfo {
        fast_requests: i32_to_u32(user_usage.gpt4_requests),
        max_fast_requests: i32_to_u32(user_usage.gpt4_max_requests),
        mtype,
        trial_days,
    })
}

// pub async fn get_available_models(auth_token: &str,checksum: &str) -> Option<Vec<Model>> {
//     let client = super::client::build_client(auth_token, checksum, CURSOR_API2_AVAILABLE_MODELS);
//     let response = client
//         .body(Vec::new())
//         .send()
//         .await
//         .ok()?
//         .bytes()
//         .await
//         .ok()?;
//     let available_models = AvailableModelsResponse::decode(response.as_ref()).ok()?;
//     Some(available_models.models.into_iter().map(|model| Model {
//         id: model.name.clone(),
//         created: CREATED,
//         object: MODEL_OBJECT,
//         owned_by: {
//             if model.name.starts_with("gpt") || model.name.starts_with("o1") {
//                 OPENAI
//             } else if model.name.starts_with("claude") {
//                 ANTHROPIC
//             } else if model.name.starts_with("gemini") {
//                 GOOGLE
//             } else {
//                 CURSOR
//             }
//         },
//     }).collect())
// }

pub async fn get_stripe_profile(auth_token: &str) -> Option<(String, u32)> {
    let client = super::client::build_profile_client(auth_token);
    let response = client.send().await.ok()?.json::<StripeProfile>().await.ok()?;
    Some((response.membership_type, i32_to_u32(response.days_remaining_on_trial)))
}
