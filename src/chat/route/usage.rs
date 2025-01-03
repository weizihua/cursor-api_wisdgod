use crate::{
    app::model::AppState,
    chat::constant::ERR_NODATA,
    common::{models::usage::GetUserInfo, utils::get_user_usage},
};
use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Deserialize)]
pub struct GetUserInfoQuery {
    alias: String,
}

pub async fn get_user_info(
    State(state): State<Arc<Mutex<AppState>>>,
    Query(query): Query<GetUserInfoQuery>,
) -> Json<GetUserInfo> {
    let token_infos = &state.lock().await.token_infos;
    let token_info = token_infos
        .iter()
        .find(|token_info| token_info.alias == Some(query.alias.clone()));

    let (auth_token, checksum) = match token_info {
        Some(token_info) => (token_info.token.clone(), token_info.checksum.clone()),
        None => return Json(GetUserInfo::Error(ERR_NODATA.to_string())),
    };

    match get_user_usage(&auth_token, &checksum).await {
        Some(usage) => Json(GetUserInfo::Usage(usage)),
        None => Json(GetUserInfo::Error(ERR_NODATA.to_string())),
    }
}
