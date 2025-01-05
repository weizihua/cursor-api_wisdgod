mod app;
mod chat;
mod common;

use app::{
    config::handle_config_update, constant::{
        EMPTY_STRING, PKG_NAME, PKG_VERSION, ROUTE_ABOUT_PATH, ROUTE_API_PATH,
        ROUTE_AUTH_CALLBACK_PATH, ROUTE_AUTH_INITIATE_PATH, ROUTE_AUTH_PATH, ROUTE_CHAT_PATH,
        ROUTE_CONFIG_PATH, ROUTE_ENV_EXAMPLE_PATH, ROUTE_GET_CHECKSUM, ROUTE_GET_TOKENINFO_PATH,
        ROUTE_GET_USER_INFO_PATH, ROUTE_HEALTH_PATH, ROUTE_LOGS_PATH, ROUTE_MODELS_PATH,
        ROUTE_README_PATH, ROUTE_ROOT_PATH, ROUTE_STATIC_PATH, ROUTE_TOKENINFO_PATH,
        ROUTE_UPDATE_TOKENINFO_PATH,
    }, db::init_database, lazy::{
        OAUTH_CLIENT_ID, OAUTH_CLIENT_SECRET, OAUTH_REDIRECT_URI, PUBLIC_AUTH_TOKEN, ROUTE_PREFIX,
    }, model::{AppConfig, AppState, VisionAbility}
};
use axum::{
    routing::{get, post},
    Router,
};
use chat::{
    route::{
        get_user_info, handle_about, handle_auth_callback, handle_auth_initiate,
        handle_config_page, handle_env_example, handle_get_checksum, handle_get_tokeninfo,
        handle_health, handle_logs, handle_logs_post, handle_readme, handle_root, handle_static,
        handle_tokeninfo_page, handle_update_tokeninfo_post,
    },
    service::{handle_chat, handle_models},
};
use common::utils::{parse_bool_from_env, parse_string_from_env};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    // 设置自定义 panic hook
    std::panic::set_hook(Box::new(|info| {
        // std::env::set_var("RUST_BACKTRACE", "1");
        if let Some(msg) = info.payload().downcast_ref::<String>() {
            eprintln!("{}", msg);
        } else if let Some(msg) = info.payload().downcast_ref::<&str>() {
            eprintln!("{}", msg);
        }
    }));

    // 加载环境变量
    dotenvy::dotenv().ok();

    if PUBLIC_AUTH_TOKEN.is_empty() {
        panic!("PUBLIC_AUTH_TOKEN must be set")
    };

    if OAUTH_CLIENT_ID.is_empty() {
        panic!("OAUTH_CLIENT_ID must be set")
    };

    if OAUTH_CLIENT_SECRET.is_empty() {
        panic!("OAUTH_CLIENT_SECRET must be set")
    };

    if OAUTH_REDIRECT_URI.is_empty() {
        panic!("OAUTH_REDIRECT_URI must be set")
    };

    // 初始化全局配置
    AppConfig::init(
        parse_bool_from_env("ENABLE_STREAM_CHECK", true),
        parse_bool_from_env("INCLUDE_STOP_REASON_STREAM", true),
        VisionAbility::from_str(&parse_string_from_env("VISION_ABILITY", EMPTY_STRING)),
        parse_bool_from_env("ENABLE_SLOW_POOL", false),
        parse_bool_from_env("PASS_ANY_CLAUDE", false),
    );

    // 初始化应用状态
    let state = Arc::new(Mutex::new(AppState::new()));

    init_database(format!("{}.db", PKG_NAME).as_str()).await.unwrap();

    // 设置路由
    let app = Router::new()
        .nest(
            ROUTE_PREFIX.as_str(),
            Router::new()
                .route(ROUTE_MODELS_PATH, get(handle_models))
                .route(ROUTE_CHAT_PATH, post(handle_chat)),
        )
        .route(ROUTE_ROOT_PATH, get(handle_root))
        .route(ROUTE_HEALTH_PATH, get(handle_health))
        .route(ROUTE_TOKENINFO_PATH, get(handle_tokeninfo_page))
        .route(ROUTE_GET_CHECKSUM, get(handle_get_checksum))
        .route(ROUTE_GET_USER_INFO_PATH, get(get_user_info))
        .route(ROUTE_GET_TOKENINFO_PATH, post(handle_get_tokeninfo))
        .route(
            ROUTE_UPDATE_TOKENINFO_PATH,
            post(handle_update_tokeninfo_post),
        )
        .route(ROUTE_LOGS_PATH, get(handle_logs))
        .route(ROUTE_LOGS_PATH, post(handle_logs_post))
        .route(ROUTE_ENV_EXAMPLE_PATH, get(handle_env_example))
        .route(ROUTE_CONFIG_PATH, get(handle_config_page))
        .route(ROUTE_CONFIG_PATH, post(handle_config_update))
        .route(ROUTE_STATIC_PATH, get(handle_static))
        .route(ROUTE_ABOUT_PATH, get(handle_about))
        .route(ROUTE_README_PATH, get(handle_readme))
        .nest(
            ROUTE_API_PATH,
            Router::new().nest(
                ROUTE_AUTH_PATH,
                Router::new()
                    .route(ROUTE_AUTH_CALLBACK_PATH, get(handle_auth_callback))
                    .route(ROUTE_AUTH_INITIATE_PATH, get(handle_auth_initiate)),
            ),
        )
        .layer(CorsLayer::permissive())
        .with_state(state);

    // 启动服务器
    let port = parse_string_from_env("PORT", "3000");
    let addr = format!("0.0.0.0:{}", port);
    println!("服务器运行在端口 {}", port);
    println!("当前版本: v{}", PKG_VERSION);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
