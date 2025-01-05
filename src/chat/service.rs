use super::constant::AVAILABLE_MODELS;
use crate::{
    app::{
        constant::{
            AUTHORIZATION_BEARER_PREFIX, CURSOR_API2_STREAM_CHAT, FINISH_REASON_STOP,
            OBJECT_CHAT_COMPLETION, OBJECT_CHAT_COMPLETION_CHUNK,
        },
        db,
        model::{AppConfig, AppState, ChatRequest, LogInfo, LogStatus, TokenStatus},
    },
    chat::{
        error::StreamError,
        model::{
            ChatResponse, Choice, Delta, Message, MessageContent, ModelsResponse, Role, Usage,
        },
        stream::{parse_stream_data, StreamMessage},
    },
    common::{
        client::build_client,
        models::{error::ChatError, ErrorResponse},
        utils::get_user_usage,
    },
};
use axum::{
    body::Body,
    extract::State,
    http::{header::CONTENT_TYPE, HeaderMap, StatusCode},
    response::Response,
    Json,
};
use bytes::Bytes;
use chrono::Local;
use futures::{Stream, StreamExt};
use rand::seq::IteratorRandom as _;
use std::{
    convert::Infallible,
    sync::{atomic::AtomicBool, Arc},
};
use std::{
    pin::Pin,
    sync::atomic::Ordering,
};
use tokio::sync::Mutex;
use uuid::Uuid;

// 模型列表处理
pub async fn handle_models() -> Json<ModelsResponse> {
    Json(ModelsResponse {
        object: "list",
        data: AVAILABLE_MODELS.to_vec(),
    })
}

// 聊天处理函数的签名
pub async fn handle_chat(
    State(state): State<Arc<Mutex<AppState>>>,
    headers: HeaderMap,
    Json(request): Json<ChatRequest>,
) -> Result<Response<Body>, (StatusCode, Json<ErrorResponse>)> {
    // 从请求头获取token
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix(AUTHORIZATION_BEARER_PREFIX))
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json(ChatError::MissingToken.to_error_response()),
        ))?;

    // 验证token并获取用户信息
    let user = db::get_user_by_auth_token(token).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ChatError::DatabaseError(err.to_string()).to_error_response()),
        )
    })?;

    let user = user.ok_or((
        StatusCode::UNAUTHORIZED,
        Json(ChatError::InvalidToken.to_error_response()),
    ))?;

    // 检查用户是否在封禁期
    if let Some(ban_expired_at) = user.ban_expired_at {
        if ban_expired_at > Local::now() {
            return Err((
                StatusCode::FORBIDDEN,
                Json(ChatError::UserBanned(ban_expired_at).to_error_response()),
            ));
        }
    }

    let tokens = db::get_available_tokens_by_user_id(Some(user.id)).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ChatError::DatabaseError(err.to_string()).to_error_response()),
        )
    })?;

    if tokens.is_empty() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ChatError::NoTokens.to_error_response()),
        ));
    }

    // 随机选择一个可用的token
    let token_info = tokens.into_iter().choose(&mut rand::thread_rng()).ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ChatError::NoTokens.to_error_response()),
    ))?;

    let allow_claude = AppConfig::get_allow_claude();
    // 验证模型是否支持并获取模型信息
    let model = AVAILABLE_MODELS
        .iter()
        .find(|m| m.id == request.model)
        .cloned();
    let model_supported = model.is_some();

    if !(model_supported || allow_claude && request.model.starts_with("claude")) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ChatError::ModelNotSupported(request.model).to_error_response()),
        ));
    }

    let request_time = Local::now();

    // 验证请求
    if request.messages.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ChatError::EmptyMessages.to_error_response()),
        ));
    }

    let log_info = LogInfo {
        id: 0, // 数据库会自动生成
        timestamp: request_time,
        token_info: token_info.clone(),
        prompt: None,
        model: request.model.clone(),
        stream: request.stream,
        status: LogStatus::Pending,
        error: None,
    };

    let log_id = db::insert_log(&log_info).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ChatError::DatabaseError(err.to_string()).to_error_response()),
        )
    })?;

    // 更新请求日志
    {
        let mut state = state.lock().await;
        state.total_requests += 1;
        state.active_requests += 1;
        let token = token_info.token.clone();
        let checksum = token_info.checksum.clone();

        // 如果有model且需要获取使用情况,创建后台任务获取
        if let Some(model) = model {
            if model.is_usage_check() {
                tokio::spawn(async move {
                    let usage = get_user_usage(&token, &checksum).await;
                    if let Err(err) = db::update_log_usage(log_id, usage) {
                        eprintln!("Failed to update log usage: {}", err);
                    }
                });
            }
        }
    }

    if db::get_user_logs_count(user.id).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ChatError::DatabaseError(err.to_string()).to_error_response()),
        )
    })? >= 100 {
        db::clean_user_logs(user.id, 100).map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ChatError::DatabaseError(err.to_string()).to_error_response()),
            )
        })?;
    }

    db::update_token_status(token_info.id, TokenStatus::Pending).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ChatError::DatabaseError(err.to_string()).to_error_response()),
        )
    })?;

    // 将消息转换为hex格式
    let hex_data = super::adapter::encode_chat_message(request.messages, &request.model)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    ChatError::RequestFailed("Failed to encode chat message".to_string())
                        .to_error_response(),
                ),
            )
        })?;

    // 构建请求客户端
    let client = build_client(&token_info.token, &token_info.checksum, CURSOR_API2_STREAM_CHAT);
    let response = client.body(hex_data).send().await;

    // 处理请求结果
    let response = match response {
        Ok(resp) => {
            // 更新请求日志为成功
            db::update_log_status(log_id, LogStatus::Success, None).map_err(|err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ChatError::DatabaseError(err.to_string()).to_error_response()),
                )
            })?;
            resp
        }
        Err(e) => {
            // 更新请求日志为失败
            db::update_log_status(log_id, LogStatus::Failed, Some(e.to_string())).map_err(|err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ChatError::DatabaseError(err.to_string()).to_error_response()),
                )
            })?;
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ChatError::RequestFailed(e.to_string()).to_error_response()),
            ));
        }
    };

    // 释放活动请求计数
    {
        let mut state = state.lock().await;
        state.active_requests -= 1;
    }

    if request.stream {
        let response_id = format!("chatcmpl-{}", Uuid::new_v4().simple());
        let full_text = Arc::new(Mutex::new(String::with_capacity(1024)));
        let is_start = Arc::new(AtomicBool::new(true));

        let stream = {
            // 创建新的 stream
            let mut stream = response.bytes_stream();

            let enable_stream_check = AppConfig::get_stream_check();

            if enable_stream_check {
                // 检查第一个 chunk
                match stream.next().await {
                    Some(first_chunk) => {
                        let chunk = first_chunk.map_err(|e| {
                            let error_message = format!("Failed to read response chunk: {}", e);
                            // 理论上，若程序正常，必定成功，因为前面判断过了
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(ChatError::RequestFailed(error_message).to_error_response()),
                            )
                        })?;

                        match parse_stream_data(&chunk) {
                            Err(StreamError::ChatError(error)) => {
                                let error_respone = error.to_error_response();
                                // 更新请求日志为失败
                                db::update_log_status(log_id, LogStatus::Failed, Some(error_respone.native_code())).map_err(|err| {
                                    (
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        Json(ChatError::DatabaseError(err.to_string()).to_error_response()),
                                    )
                                })?;
                                return Err((
                                    error_respone.status_code(),
                                    Json(error_respone.to_common()),
                                ));
                            }
                            Ok(_) | Err(_) => {
                                // 创建一个包含第一个 chunk 的 stream
                                Box::pin(
                                    futures::stream::once(async move { Ok(chunk) }).chain(stream),
                                )
                                    as Pin<
                                        Box<
                                            dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send,
                                        >,
                                    >
                            }
                        }
                    }
                    None => {
                        // Box::pin(stream)
                        //     as Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>
                        // 更新请求日志为失败
                        db::update_log_status(log_id, LogStatus::Failed, Some("Empty stream response".to_string())).map_err(|err| {
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(ChatError::DatabaseError(err.to_string()).to_error_response()),
                            )
                        })?;
                        return Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(
                                ChatError::RequestFailed("Empty stream response".to_string())
                                    .to_error_response(),
                            ),
                        ));
                    }
                }
            } else {
                Box::pin(stream)
                    as Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>
            }
        }
        .then({
            let buffer = Arc::new(Mutex::new(Vec::new())); // 创建共享的buffer

            move |chunk| {
                let buffer = buffer.clone();
                let response_id = response_id.clone();
                let model = request.model.clone();
                let is_start = is_start.clone();
                let full_text = full_text.clone();

                async move {
                    let chunk = chunk.unwrap_or_default();
                    let mut buffer_guard = buffer.lock().await;
                    buffer_guard.extend_from_slice(&chunk);

                    match parse_stream_data(&buffer_guard) {
                        Ok(StreamMessage::Content(texts)) => {
                            buffer_guard.clear();
                            let mut response_data = String::new();

                            for text in texts {
                                let mut text_guard = full_text.lock().await;
                                text_guard.push_str(&text);
                                let is_first = is_start.load(Ordering::SeqCst);

                                let response = ChatResponse {
                                    id: response_id.clone(),
                                    object: OBJECT_CHAT_COMPLETION_CHUNK.to_string(),
                                    created: chrono::Utc::now().timestamp(),
                                    model: if is_first { Some(model.clone()) } else { None },
                                    choices: vec![Choice {
                                        index: 0,
                                        message: None,
                                        delta: Some(Delta {
                                            role: if is_first {
                                                is_start.store(false, Ordering::SeqCst);
                                                Some(Role::Assistant)
                                            } else {
                                                None
                                            },
                                            content: Some(text),
                                        }),
                                        finish_reason: None,
                                    }],
                                    usage: None,
                                };

                                response_data.push_str(&format!(
                                    "data: {}\n\n",
                                    serde_json::to_string(&response).unwrap()
                                ));
                            }

                            Ok::<_, Infallible>(Bytes::from(response_data))
                        }
                        Ok(StreamMessage::StreamStart) => {
                            buffer_guard.clear();
                            // 发送初始响应，包含模型信息
                            let response = ChatResponse {
                                id: response_id.clone(),
                                object: OBJECT_CHAT_COMPLETION_CHUNK.to_string(),
                                created: chrono::Utc::now().timestamp(),
                                model: {
                                    is_start.store(true, Ordering::SeqCst);
                                    Some(model.clone())
                                },
                                choices: vec![Choice {
                                    index: 0,
                                    message: None,
                                    delta: Some(Delta {
                                        role: Some(Role::Assistant),
                                        content: Some(String::new()),
                                    }),
                                    finish_reason: None,
                                }],
                                usage: None,
                            };

                            Ok(Bytes::from(format!(
                                "data: {}\n\n",
                                serde_json::to_string(&response).unwrap()
                            )))
                        }
                        Ok(StreamMessage::StreamEnd) => {
                            buffer_guard.clear();
                            // 根据配置决定是否发送最后的 finish_reason
                            let include_finish_reason = AppConfig::get_stop_stream();

                            if include_finish_reason {
                                let response = ChatResponse {
                                    id: response_id.clone(),
                                    object: OBJECT_CHAT_COMPLETION_CHUNK.to_string(),
                                    created: chrono::Utc::now().timestamp(),
                                    model: None,
                                    choices: vec![Choice {
                                        index: 0,
                                        message: None,
                                        delta: Some(Delta {
                                            role: None,
                                            content: None,
                                        }),
                                        finish_reason: Some(FINISH_REASON_STOP.to_string()),
                                    }],
                                    usage: None,
                                };
                                Ok(Bytes::from(format!(
                                    "data: {}\n\ndata: [DONE]\n\n",
                                    serde_json::to_string(&response).unwrap()
                                )))
                            } else {
                                Ok(Bytes::from("data: [DONE]\n\n"))
                            }
                        }
                        Ok(StreamMessage::Incomplete) => {
                            // 保持buffer中的数据以待下一个chunk
                            Ok(Bytes::new())
                        }
                        Ok(StreamMessage::Debug(debug_prompt)) => {
                            buffer_guard.clear();
                            if let Err(err) = db::update_log_prompt(log_id, Some(debug_prompt.clone())) {
                                eprintln!("Failed to update log prompt: {}", err);
                            }
                            Ok(Bytes::new())
                        }
                        Err(e) => {
                            buffer_guard.clear();
                            eprintln!("[警告] Stream error: {}", e);
                            Ok(Bytes::new())
                        }
                    }
                }
            }
        });

        Ok(Response::builder()
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .header(CONTENT_TYPE, "text/event-stream")
            .body(Body::from_stream(stream))
            .unwrap())
    } else {
        // 非流式响应
        let mut full_text = String::with_capacity(1024); // 预分配合适的容量
        let mut stream = response.bytes_stream();
        let mut prompt = None;

        let mut buffer = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(
                        ChatError::RequestFailed(format!("Failed to read response chunk: {}", e))
                            .to_error_response(),
                    ),
                )
            })?;

            buffer.extend_from_slice(&chunk);

            match parse_stream_data(&buffer) {
                Ok(StreamMessage::Content(texts)) => {
                    for text in texts {
                        full_text.push_str(&text);
                    }
                    buffer.clear();
                }
                Ok(StreamMessage::Incomplete) => {
                    continue;
                }
                Ok(StreamMessage::Debug(debug_prompt)) => {
                    prompt = Some(debug_prompt);
                    buffer.clear();
                }
                Ok(StreamMessage::StreamStart) | Ok(StreamMessage::StreamEnd) => {
                    buffer.clear();
                }
                Err(StreamError::ChatError(error)) => {
                    return Err((
                        StatusCode::from_u16(error.status_code())
                            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                        Json(error.to_error_response().to_common()),
                    ));
                }
                Err(_) => {
                    buffer.clear();
                    continue;
                }
            }
        }

        let prompt_tokens = prompt.as_ref().map(|p| p.len() as u32).unwrap_or(0);
        let completion_tokens = full_text.len() as u32;
        let total_tokens = prompt_tokens + completion_tokens;

        // 检查响应是否为空
        if full_text.is_empty() {
            // 更新请求日志为失败
            db::update_log_status(log_id, LogStatus::Failed, Some("Empty response received".to_string())).map_err(|err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ChatError::DatabaseError(err.to_string()).to_error_response()),
                )
            })?;
            db::update_log_prompt(log_id, None).map_err(|err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ChatError::DatabaseError(err.to_string()).to_error_response()),
                )
            })?;
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    ChatError::RequestFailed("Empty response received".to_string())
                        .to_error_response(),
                ),
            ));
        }

        // 更新请求日志提示词
        db::update_log_prompt(log_id, prompt).map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ChatError::DatabaseError(err.to_string()).to_error_response()),
            )
        })?;

        let response_data = ChatResponse {
            id: format!("chatcmpl-{}", Uuid::new_v4().simple()),
            object: OBJECT_CHAT_COMPLETION.to_string(),
            created: chrono::Utc::now().timestamp(),
            model: Some(request.model),
            choices: vec![Choice {
                index: 0,
                message: Some(Message {
                    role: Role::Assistant,
                    content: MessageContent::Text(full_text),
                }),
                delta: None,
                finish_reason: Some(FINISH_REASON_STOP.to_string()),
            }],
            usage: Some(Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens,
            }),
        };

        Ok(Response::builder()
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_string(&response_data).unwrap()))
            .unwrap())
    }
}
