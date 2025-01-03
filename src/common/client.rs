use crate::app::{
    constant::{
        AUTHORIZATION_BEARER_PREFIX, CONTENT_TYPE_CONNECT_PROTO, CONTENT_TYPE_PROTO,
        CURSOR_API2_STREAM_CHAT, HEADER_NAME_AUTHORIZATION,
        HEADER_NAME_CONTENT_TYPE,
    },
    lazy::{CURSOR_API2_HOST, CURSOR_API2_BASE_URL},
};
use reqwest::Client;
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
        .header(HEADER_NAME_CONTENT_TYPE, content_type)
        .header(
            HEADER_NAME_AUTHORIZATION,
            format!("{}{}", AUTHORIZATION_BEARER_PREFIX, auth_token),
        )
        .header("connect-accept-encoding", "gzip,br")
        .header("connect-protocol-version", "1")
        .header("user-agent", "connect-es/1.6.1")
        .header("x-amzn-trace-id", format!("Root={}", trace_id))
        .header("x-cursor-checksum", checksum)
        .header("x-cursor-client-version", "0.42.5")
        .header("x-cursor-timezone", "Asia/Shanghai")
        .header("x-ghost-mode", "false")
        .header("x-request-id", trace_id)
        .header("Host", CURSOR_API2_HOST.clone())
}
