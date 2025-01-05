use crate::{
    app::{
        constant::{
            AUTHORIZATION_BEARER_PREFIX, CONTENT_TYPE_TEXT_HTML_WITH_UTF8,
            CONTENT_TYPE_TEXT_PLAIN_WITH_UTF8, ROUTE_LOGS_PATH,
        },
        db,
        model::{AppConfig, LogInfo, PageContent},
    },
    common::models::ApiStatus,
};
use axum::{
    body::Body,
    http::{header::{AUTHORIZATION, CONTENT_TYPE}, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Local;

// 日志处理
pub async fn handle_logs() -> impl IntoResponse {
    match AppConfig::get_page_content(ROUTE_LOGS_PATH).unwrap_or_default() {
        PageContent::Default => Response::builder()
            .header(CONTENT_TYPE, CONTENT_TYPE_TEXT_HTML_WITH_UTF8)
            .body(Body::from(
                include_str!("../../../static/logs.min.html").to_string(),
            ))
            .unwrap(),
        PageContent::Text(content) => Response::builder()
            .header(CONTENT_TYPE, CONTENT_TYPE_TEXT_PLAIN_WITH_UTF8)
            .body(Body::from(content.clone()))
            .unwrap(),
        PageContent::Html(content) => Response::builder()
            .header(CONTENT_TYPE, CONTENT_TYPE_TEXT_HTML_WITH_UTF8)
            .body(Body::from(content.clone()))
            .unwrap(),
    }
}

pub async fn handle_logs_post(headers: HeaderMap) -> Result<Json<LogsResponse>, StatusCode> {
    // 验证 auth_token
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix(AUTHORIZATION_BEARER_PREFIX))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // 通过 auth_token 获取用户信息
    let user = db::get_user_by_auth_token(auth_header)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // 获取用户的日志记录
    let logs =
        db::get_logs_by_user_id(Some(user.id)).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(LogsResponse {
        status: ApiStatus::Success,
        total: logs.len(),
        logs,
        timestamp: Local::now().to_string(),
    }))
}

#[derive(serde::Serialize)]
pub struct LogsResponse {
    pub status: ApiStatus,
    pub total: usize,
    pub logs: Vec<LogInfo>,
    pub timestamp: String,
}
