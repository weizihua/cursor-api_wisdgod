use crate::app::constant::ROUTE_TOKENINFO_PATH;
use crate::app::lazy::{OAUTH_CLIENT_ID, OAUTH_CLIENT_SECRET, OAUTH_REDIRECT_URI};
use crate::common::utils::oauth::ForumOAuth;
use axum::http::{header::SET_COOKIE, HeaderMap};
use axum::{extract::Query, response::Redirect};
use base64::{engine::general_purpose::URL_SAFE, Engine};
use ring::rand::SecureRandom as _;
use ring::{aead, rand};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
pub struct AuthCallback {
    code: String,
    state: String,
}

// 用于加密的密钥，使用 OnceLock 确保只初始化一次
static ENCRYPTION_KEY: OnceLock<aead::LessSafeKey> = OnceLock::new();

// 初始化加密密钥
fn get_encryption_key() -> &'static aead::LessSafeKey {
    ENCRYPTION_KEY.get_or_init(|| {
        let rng = rand::SystemRandom::new();
        let mut key_bytes = [0u8; 32];
        rng.fill(&mut key_bytes).unwrap();
        let key = aead::UnboundKey::new(&aead::CHACHA20_POLY1305, &key_bytes).unwrap();
        aead::LessSafeKey::new(key)
    })
}

// 加密 state
fn encrypt_state(state: &str) -> Result<String, String> {
    let key = get_encryption_key();
    let nonce = aead::Nonce::assume_unique_for_key([0; 12]); // 在生产环境中应使用随机 nonce
    let mut in_out = state.as_bytes().to_vec();
    key.seal_in_place_append_tag(nonce, aead::Aad::empty(), &mut in_out)
        .map_err(|e| e.to_string())?;
    Ok(URL_SAFE.encode(in_out))
}

// 解密 state
fn decrypt_state(encrypted_state: &str) -> Result<String, String> {
    let key = get_encryption_key();
    let nonce = aead::Nonce::assume_unique_for_key([0; 12]);
    let mut encrypted_data = URL_SAFE
        .decode(encrypted_state)
        .map_err(|e| e.to_string())?;
    let decrypted = key
        .open_in_place(nonce, aead::Aad::empty(), &mut encrypted_data)
        .map_err(|e| e.to_string())?;
    String::from_utf8(decrypted.to_vec()).map_err(|e| e.to_string())
}

pub async fn handle_auth_callback(
    headers: HeaderMap,
    Query(params): Query<AuthCallback>,
) -> Result<Redirect, String> {
    let cookie_header = headers
        .get("cookie")
        .ok_or_else(|| "Missing cookie header".to_string())?;

    let cookie_str = cookie_header.to_str().map_err(|e| e.to_string())?;

    let encrypted_state = cookie_str
        .split(';')
        .find(|s| s.trim().starts_with("oauth_state="))
        .and_then(|s| s.trim().strip_prefix("oauth_state="))
        .ok_or_else(|| "Missing state cookie".to_string())?;

    // 解密 state
    let expected_state = decrypt_state(encrypted_state)?;

    let oauth = ForumOAuth::new(
        OAUTH_CLIENT_ID.to_string(),
        OAUTH_CLIENT_SECRET.to_string(),
        OAUTH_REDIRECT_URI.to_string(),
    )
    .map_err(|e| e.to_string())?;

    let access_token = oauth
        .exchange_code_for_token(&params.code, &params.state, &expected_state)
        .await
        .map_err(|e| e.to_string())?;

    let redirect_url = format!("{}?auth={}", ROUTE_TOKENINFO_PATH, access_token);
    Ok(Redirect::to(&redirect_url))
}

pub async fn handle_auth_initiate() -> Result<(HeaderMap, Redirect), String> {
    let oauth = ForumOAuth::new(
        OAUTH_CLIENT_ID.to_string(),
        OAUTH_CLIENT_SECRET.to_string(),
        OAUTH_REDIRECT_URI.to_string(),
    )
    .map_err(|e| e.to_string())?;

    let (auth_url, state) = oauth.get_authorize_url();

    // 加密 state
    let encrypted_state = encrypt_state(state.secret())?;

    // 创建安全的 cookie
    let cookie = format!(
        "oauth_state={}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=300",
        encrypted_state
    );

    let mut headers = HeaderMap::new();
    headers.insert(SET_COOKIE, cookie.parse().unwrap());

    Ok((headers, Redirect::to(&auth_url.to_string())))
}
