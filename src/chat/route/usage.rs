use crate::{
    app::{
        constant::AUTHORIZATION_BEARER_PREFIX,
        db::{get_token_by_alias_and_user, get_user_by_auth_token},
    },
    chat::constant::ERR_NODATA,
    common::{models::usage::GetUserInfo, utils::get_user_usage},
};
use axum::{
    extract::Query,
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct GetUserInfoQuery {
    alias: String,
    user_id: Option<i64>,
}

pub async fn get_user_info(
    headers: HeaderMap,
    Query(query): Query<GetUserInfoQuery>,
) -> Result<Json<GetUserInfo>, StatusCode> {
    // 1. 验证用户身份
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix(AUTHORIZATION_BEARER_PREFIX))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // 2. 获取用户信息
    let user = get_user_by_auth_token(auth_header)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // 3. 查询token信息
    let token_info = match get_token_by_alias_and_user(
        &query.alias,
        user.id,
        query.user_id
    ) {
        Ok(Some(token)) => token,
        Ok(None) => return Ok(Json(GetUserInfo::Error(ERR_NODATA.to_string()))),
        Err(_) => return Ok(Json(GetUserInfo::Error("Database error".to_string()))),
    };

    // 4. 获取使用情况
    match get_user_usage(&token_info.token, &token_info.checksum).await {
        Some(usage) => Ok(Json(GetUserInfo::Usage(usage))),
        None => Ok(Json(GetUserInfo::Error(ERR_NODATA.to_string()))),
    }
}
