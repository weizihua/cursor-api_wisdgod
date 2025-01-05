use crate::{
    app::{
        constant::{
            AUTHORIZATION_BEARER_PREFIX, CONTENT_TYPE_TEXT_HTML_WITH_UTF8,
            CONTENT_TYPE_TEXT_PLAIN_WITH_UTF8, ROUTE_TOKENINFO_PATH,
        },
        db::{
            get_token_by_id, get_token_by_token, get_tokens_by_user_id, get_user_by_auth_token,
            insert_token, update_token,
        },
        model::{AppConfig, PageContent, TokenInfo, TokenStatus, TokenUpdateRequest},
    },
    common::{
        models::ApiStatus,
        utils::{extract_user_id, extract_time, generate_checksum, generate_hash, validate_checksum},
    },
};
use axum::{
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE},
        HeaderMap,
    },
    response::{IntoResponse, Response},
    Json,
};
use chrono::Local;
use reqwest::StatusCode;
use serde::Serialize;

#[derive(Serialize)]
pub struct ChecksumResponse {
    pub checksum: String,
}

pub async fn handle_get_checksum() -> Json<ChecksumResponse> {
    let checksum = generate_checksum(&generate_hash(), Some(&generate_hash()));
    Json(ChecksumResponse { checksum })
}

// 获取 TokenInfo 处理
pub async fn handle_get_tokeninfo(
    headers: HeaderMap,
) -> Result<Json<TokenInfoResponse>, StatusCode> {
    // 验证用户身份
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix(AUTHORIZATION_BEARER_PREFIX))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // 获取用户信息
    let user = get_user_by_auth_token(auth_header)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // 获取用户的tokens
    let tokens = if user.id == 0 {
        // 管理员可以查看所有tokens
        get_tokens_by_user_id(None)
    } else {
        // 普通用户只能查看自己的tokens
        get_tokens_by_user_id(Some(user.id))
    }
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let token_num = tokens.len();

    Ok(Json(TokenInfoResponse {
        status: ApiStatus::Success,
        tokens: Some(tokens),
        num: Some(token_num),
        message: None,
    }))
}

#[derive(Serialize)]
pub struct TokenInfoResponse {
    pub status: ApiStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<Vec<TokenInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

pub async fn handle_update_tokeninfo_post(
    headers: HeaderMap,
    Json(request): Json<TokenUpdateRequest>,
) -> Result<Json<TokenInfoResponse>, StatusCode> {
    // 验证用户身份
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix(AUTHORIZATION_BEARER_PREFIX))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // 获取用户信息
    let user = get_user_by_auth_token(auth_header)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if !validate_checksum(&request.checksum) {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 检查token是否已存在
    let existing_token =
        get_token_by_token(&request.token).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let is_update = existing_token.is_some();

    let token_info = match existing_token {
        Some(mut token) => {
            // 更新现有token
            token.checksum = request.checksum;
            token.alias = request.alias;
            token.is_public = request.is_public;

            // 更新数据库
            update_token(&token).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            token
        }
        None => {
            let now = Local::now();
            let alias = if request.alias.is_none() {
                match extract_user_id(&request.token) {
                    Some(user_id) => Some(user_id),
                    None => None,
                }
            } else {
                request.alias
            };
            // 创建新token
            let new_token = TokenInfo {
                id: 0, // 数据库会自动分配ID
                create_at: extract_time(&request.token).unwrap_or_else(|| now),
                token: request.token,
                checksum: request.checksum,
                alias,
                status: TokenStatus::Active,
                pengding_at: now,
                user_id: user.id,
                is_public: request.is_public,
                usage: None,
            };

            // 插入数据库
            let id = insert_token(&new_token).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            // 获取插入后的完整token信息
            get_token_by_id(id)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?
        }
    };

    Ok(Json(TokenInfoResponse {
        status: ApiStatus::Success,
        tokens: None,
        num: None,
        message: Some(format!(
            "Token {} has been {}",
            token_info.token,
            if is_update {
                "updated"
            } else {
                "created"
            }
        )),
    }))
}

pub async fn handle_tokeninfo_page() -> impl IntoResponse {
    match AppConfig::get_page_content(ROUTE_TOKENINFO_PATH).unwrap_or_default() {
        PageContent::Default => Response::builder()
            .header(CONTENT_TYPE, CONTENT_TYPE_TEXT_HTML_WITH_UTF8)
            .body(include_str!("../../../static/tokeninfo.min.html").to_string())
            .unwrap(),
        PageContent::Text(content) => Response::builder()
            .header(CONTENT_TYPE, CONTENT_TYPE_TEXT_PLAIN_WITH_UTF8)
            .body(content.clone())
            .unwrap(),
        PageContent::Html(content) => Response::builder()
            .header(CONTENT_TYPE, CONTENT_TYPE_TEXT_HTML_WITH_UTF8)
            .body(content.clone())
            .unwrap(),
    }
}
