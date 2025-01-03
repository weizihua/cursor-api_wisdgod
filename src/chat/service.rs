use super::constant::AVAILABLE_MODELS;
use crate::{
    app::{
        constant::{
            AUTHORIZATION_BEARER_PREFIX, CURSOR_API2_STREAM_CHAT, FINISH_REASON_STOP,
            HEADER_NAME_CONTENT_TYPE, OBJECT_CHAT_COMPLETION, OBJECT_CHAT_COMPLETION_CHUNK,
            STATUS_FAILED, STATUS_SUCCESS,
        },
        model::{AppConfig, AppState, ChatRequest, RequestLog, TokenInfo},
        lazy::AUTH_TOKEN,
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
    http::{HeaderMap, StatusCode},
    response::Response,
    Json,
};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use std::{
    convert::Infallible,
    sync::{atomic::AtomicBool, Arc},
};
use std::{
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
};
use tokio::sync::Mutex;
use uuid::Uuid;

// 模型列表处理
pub async fn handle_models() -> Json<ModelsResponse> {
    Json(ModelsResponse {
        object: "list",
        data: &AVAILABLE_MODELS,
    })
}

// 聊天处理函数的签名
pub async fn handle_chat(
    State(state): State<Arc<Mutex<AppState>>>,
    headers: HeaderMap,
    Json(request): Json<ChatRequest>,
) -> Result<Response<Body>, (StatusCode, Json<ErrorResponse>)> {
    let allow_claude = AppConfig::get_allow_claude();
    // 验证模型是否支持并获取模型信息
    let model = AVAILABLE_MODELS.iter().find(|m| m.id == request.model);
    let model_supported = model.is_some();

    if !(model_supported || allow_claude && request.model.starts_with("claude")) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ChatError::ModelNotSupported(request.model).to_json()),
        ));
    }

    let request_time = chrono::Local::now();

    // 验证请求
    if request.messages.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ChatError::EmptyMessages.to_json()),
        ));
    }

    // 获取并处理认证令牌
    let auth_token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix(AUTHORIZATION_BEARER_PREFIX))
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json(ChatError::Unauthorized.to_json()),
        ))?;

    // 验证 AuthToken
    if auth_token != AUTH_TOKEN.as_str() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ChatError::Unauthorized.to_json()),
        ));
    }

    // 完整的令牌处理逻辑和对应的 checksum
    let (auth_token, checksum, alias) = {
        static CURRENT_KEY_INDEX: AtomicUsize = AtomicUsize::new(0);
        let state_guard = state.lock().await;
        let token_infos = &state_guard.token_infos;

        if token_infos.is_empty() {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ChatError::NoTokens.to_json()),
            ));
        }

        let index = CURRENT_KEY_INDEX.fetch_add(1, Ordering::SeqCst) % token_infos.len();
        let token_info = &token_infos[index];
        (
            token_info.token.clone(),
            token_info.checksum.clone(),
            token_info.alias.clone(),
        )
    };

    // 更新请求日志
    {
        let state_clone = state.clone();
        let mut state = state.lock().await;
        state.total_requests += 1;
        state.active_requests += 1;

        // 如果有model且需要获取使用情况,创建后台任务获取
        if let Some(model) = model {
            if model.is_usage_check() {
                let auth_token_clone = auth_token.clone();
                let checksum_clone = checksum.clone();
                let state_clone = state_clone.clone();

                tokio::spawn(async move {
                    let usage = get_user_usage(&auth_token_clone, &checksum_clone).await;
                    let mut state = state_clone.lock().await;
                    // 根据时间戳找到对应的日志
                    if let Some(log) = state
                        .request_logs
                        .iter_mut()
                        .find(|log| log.timestamp == request_time)
                    {
                        log.token_info.usage = usage;
                    }
                });
            }
        }

        let next_id = state.request_logs.last().map_or(1, |log| log.id + 1);
        state.request_logs.push(RequestLog {
            id: next_id,
            timestamp: request_time,
            model: request.model.clone(),
            token_info: TokenInfo {
                token: auth_token.clone(),
                checksum: checksum.clone(),
                alias: alias.clone(),
                usage: None,
            },
            prompt: None,
            stream: request.stream,
            status: "pending",
            error: None,
        });

        if state.request_logs.len() > 100 {
            state.request_logs.remove(0);
        }
    }

    // 将消息转换为hex格式
    let hex_data = super::adapter::encode_chat_message(request.messages, &request.model)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    ChatError::RequestFailed("Failed to encode chat message".to_string()).to_json(),
                ),
            )
        })?;

    // 构建请求客户端
    let client = build_client(&auth_token, &checksum, CURSOR_API2_STREAM_CHAT);
    let response = client.body(hex_data).send().await;

    // 处理请求结果
    let response = match response {
        Ok(resp) => {
            // 更新请求日志为成功
            {
                let mut state = state.lock().await;
                state.request_logs.last_mut().unwrap().status = STATUS_SUCCESS;
            }
            resp
        }
        Err(e) => {
            // 更新请求日志为失败
            {
                let mut state = state.lock().await;
                if let Some(last_log) = state.request_logs.last_mut() {
                    last_log.status = STATUS_FAILED;
                    last_log.error = Some(e.to_string());
                }
            }
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ChatError::RequestFailed(e.to_string()).to_json()),
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
                                Json(ChatError::RequestFailed(error_message).to_json()),
                            )
                        })?;

                        match parse_stream_data(&chunk) {
                            Err(StreamError::ChatError(error)) => {
                                let error_respone = error.to_error_response();
                                // 更新请求日志为失败
                                {
                                    let mut state = state.lock().await;
                                    if let Some(last_log) = state.request_logs.last_mut() {
                                        last_log.status = STATUS_FAILED;
                                        last_log.error = Some(error_respone.native_code());
                                    }
                                }
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
                        {
                            let mut state = state.lock().await;
                            if let Some(last_log) = state.request_logs.last_mut() {
                                last_log.status = STATUS_FAILED;
                                last_log.error = Some("Empty stream response".to_string());
                            }
                        }
                        return Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(
                                ChatError::RequestFailed("Empty stream response".to_string())
                                    .to_json(),
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
                let state = state.clone();

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
                            if let Ok(mut state) = state.try_lock() {
                                if let Some(last_log) = state.request_logs.last_mut() {
                                    last_log.prompt = Some(debug_prompt.clone());
                                }
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
            .header(HEADER_NAME_CONTENT_TYPE, "text/event-stream")
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
                            .to_json(),
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
            {
                let mut state = state.lock().await;
                if let Some(last_log) = state.request_logs.last_mut() {
                    last_log.status = STATUS_FAILED;
                    last_log.error = Some("Empty response received".to_string());
                    if let Some(p) = prompt {
                        last_log.prompt = Some(p);
                    }
                }
            }
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ChatError::RequestFailed("Empty response received".to_string()).to_json()),
            ));
        }

        // 更新请求日志提示词
        {
            let mut state = state.lock().await;
            if let Some(last_log) = state.request_logs.last_mut() {
                last_log.prompt = prompt;
            }
        }

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
            .header(HEADER_NAME_CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_string(&response_data).unwrap()))
            .unwrap())
    }
}
