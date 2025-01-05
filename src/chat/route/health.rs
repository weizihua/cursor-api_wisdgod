use crate::{
    app::{
        constant::{
            AUTHORIZATION_BEARER_PREFIX, CONTENT_TYPE_TEXT_HTML_WITH_UTF8,
            CONTENT_TYPE_TEXT_PLAIN_WITH_UTF8, PKG_VERSION, ROUTE_HEALTH_PATH, ROUTE_ROOT_PATH,
        },
        db,
        lazy::get_start_time,
        model::{AppConfig, AppState, PageContent},
    },
    chat::constant::AVAILABLE_MODELS,
    common::models::{
        health::{CpuInfo, HealthCheckResponse, MemoryInfo, SystemInfo, SystemStats},
        ApiStatus,
    },
};
use axum::{
    body::Body,
    extract::State,
    http::{
        header::{CONTENT_TYPE, LOCATION},
        HeaderMap, StatusCode,
    },
    response::{IntoResponse, Response},
    Json,
};
use chrono::Local;
use std::sync::Arc;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use tokio::sync::Mutex;

pub async fn handle_root() -> impl IntoResponse {
    match AppConfig::get_page_content(ROUTE_ROOT_PATH).unwrap_or_default() {
        PageContent::Default => Response::builder()
            .status(StatusCode::TEMPORARY_REDIRECT)
            .header(LOCATION, ROUTE_HEALTH_PATH)
            .body(Body::empty())
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

pub async fn handle_health(
    headers: HeaderMap,
    State(state): State<Arc<Mutex<AppState>>>,
) -> Json<HealthCheckResponse> {
    let start_time = get_start_time();

    // 尝试从请求头获取token并验证用户
    let mut stats = None;
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix(AUTHORIZATION_BEARER_PREFIX));

    if let Some(token) = token {
        if let Ok(Some(user)) = db::get_user_by_auth_token(token) {
            if user.id == 0 && user.ban_expired_at.map_or(true, |t| t <= Local::now()) {
                // 创建系统信息实例，只监控 CPU 和内存
                let mut sys = System::new_with_specifics(
                    RefreshKind::nothing()
                        .with_memory(MemoryRefreshKind::everything())
                        .with_cpu(CpuRefreshKind::everything()),
                );

                std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);

                // 刷新 CPU 和内存信息
                sys.refresh_memory();
                sys.refresh_cpu_usage();

                let pid = std::process::id() as usize;
                let process = sys.process(pid.into());

                // 获取内存信息
                let memory = process.map(|p| p.memory()).unwrap_or(0);

                // 获取 CPU 使用率
                let cpu_usage = sys.global_cpu_usage();

                let state = state.lock().await;

                stats = Some(SystemStats {
                    started: start_time.to_string(),
                    total_requests: state.total_requests,
                    active_requests: state.active_requests,
                    system: SystemInfo {
                        memory: MemoryInfo {
                            rss: memory, // 物理内存使用量(字节)
                        },
                        cpu: CpuInfo {
                            usage: cpu_usage, // CPU 使用率(百分比)
                        },
                    },
                });
            }
        }
    }

    Json(HealthCheckResponse {
        status: ApiStatus::Healthy,
        version: PKG_VERSION,
        uptime: (Local::now() - start_time).num_seconds(),
        stats,
        models: AVAILABLE_MODELS
            .iter()
            .map(|m| m.id.clone())
            .collect::<Vec<_>>(),
    })
}
