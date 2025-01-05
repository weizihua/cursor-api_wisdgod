use crate::app::{
    constant::{
        AUTHORIZATION_BEARER_PREFIX, CONTENT_TYPE_CONNECT_PROTO, CONTENT_TYPE_PROTO,
        CURSOR_API2_STREAM_CHAT, HEADER_NAME_GHOST_MODE,
        TRUE, FALSE
    },
    lazy::{CURSOR_API2_BASE_URL, CURSOR_API2_HOST},
};
use reqwest::{header::{CONTENT_TYPE,AUTHORIZATION,USER_AGENT,HOST}, Client};
use uuid::Uuid;

/// 返回预构建的 Cursor API 客户端
pub fn build_client(auth_token: &str, checksum: &str, endpoint: &str) -> reqwest::RequestBuilder {
    let client = Client::new();
    let trace_id = Uuid::new_v4().to_string();
    let content_type = if endpoint == CURSOR_API2_STREAM_CHAT {
        CONTENT_TYPE_CONNECT_PROTO
    } else {
        CONTENT_TYPE_PROTO
    };

    client
        .post(format!("{}{}", *CURSOR_API2_BASE_URL, endpoint))
        .header(CONTENT_TYPE, content_type)
        .header(
            AUTHORIZATION,
            format!("{}{}", AUTHORIZATION_BEARER_PREFIX, auth_token),
        )
        .header("connect-accept-encoding", "gzip,br")
        .header("connect-protocol-version", "1")
        .header(USER_AGENT, "connect-es/1.6.1")
        .header("x-amzn-trace-id", format!("Root={}", trace_id))
        .header("x-cursor-checksum", checksum)
        .header("x-cursor-client-version", "0.42.5")
        .header("x-cursor-timezone", "Asia/Shanghai")
        .header(HEADER_NAME_GHOST_MODE, FALSE)
        .header("x-request-id", trace_id)
        .header(HOST, CURSOR_API2_HOST.clone())
}

/// 返回预构建的获取 Stripe 账户信息的 Cursor API 客户端
pub fn build_profile_client(auth_token: &str) -> reqwest::RequestBuilder {
    let client = Client::new();

    client
        .get(format!("{}/auth/full_stripe_profile", *CURSOR_API2_BASE_URL))
        .header(HOST, CURSOR_API2_HOST.clone())
        .header("sec-ch-ua", "\"Not-A.Brand\";v=\"99\", \"Chromium\";v=\"124\"")
        .header(HEADER_NAME_GHOST_MODE, TRUE) 
        .header("sec-ch-ua-mobile", "?0")
        .header(
            AUTHORIZATION,
            format!("{}{}", AUTHORIZATION_BEARER_PREFIX, auth_token),
        )
        .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Cursor/0.42.5 Chrome/124.0.6367.243 Electron/30.4.0 Safari/537.36")
        .header("sec-ch-ua-platform", "\"Windows\"")
        .header("accept", "*/*")
        .header("origin", "vscode-file://vscode-app")
        .header("sec-fetch-site", "cross-site")
        .header("sec-fetch-mode", "cors") 
        .header("sec-fetch-dest", "empty")
        .header("accept-encoding", "gzip, deflate, br")
        .header("accept-language", "zh-CN")
        .header("priority", "u=1, i")
}
