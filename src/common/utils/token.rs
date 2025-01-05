use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Local, TimeZone};

// 验证jwt token是否有效
pub fn validate_token(token: &str) -> bool {
    // 检查 token 格式
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return false;
    }

    // 解码 payload
    let payload = match URL_SAFE_NO_PAD.decode(parts[1]) {
        Ok(decoded) => decoded,
        Err(_) => return false,
    };

    // 转换为字符串
    let payload_str = match String::from_utf8(payload) {
        Ok(s) => s,
        Err(_) => return false,
    };

    // 解析 JSON
    let payload_json: serde_json::Value = match serde_json::from_str(&payload_str) {
        Ok(v) => v,
        Err(_) => return false,
    };

    // 验证必要字段是否存在且有效
    let required_fields = ["sub", "exp", "iss", "aud", "randomness", "time"];
    for field in required_fields {
        if !payload_json.get(field).is_some() {
            return false;
        }
    }

    // 验证 randomness 长度
    if let Some(randomness) = payload_json["randomness"].as_str() {
        if randomness.len() != 18 {
            return false;
        }
    } else {
        return false;
    }

    // 验证 time 字段
    if let Some(time) = payload_json["time"].as_str() {
        // 验证 time 是否为有效的数字字符串
        if let Ok(time_value) = time.parse::<i64>() {
            let current_time = chrono::Utc::now().timestamp();
            if time_value > current_time {
                return false;
            }
        } else {
            return false;
        }
    } else {
        return false;
    }

    // 验证过期时间
    if let Some(exp) = payload_json["exp"].as_i64() {
        let current_time = chrono::Utc::now().timestamp();
        if current_time > exp {
            return false;
        }
    } else {
        return false;
    }

    // 验证发行者
    if payload_json["iss"].as_str() != Some("https://authentication.cursor.sh") {
        return false;
    }

    // 验证受众
    if payload_json["aud"].as_str() != Some("https://cursor.com") {
        return false;
    }

    true
}

// 从 JWT token 中提取用户 ID
pub fn extract_user_id(token: &str) -> Option<String> {
    // JWT token 由3部分组成，用 . 分隔
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    // 解码 payload (第二部分)
    let payload = match URL_SAFE_NO_PAD.decode(parts[1]) {
        Ok(decoded) => decoded,
        Err(_) => return None,
    };

    // 将 payload 转换为字符串
    let payload_str = match String::from_utf8(payload) {
        Ok(s) => s,
        Err(_) => return None,
    };

    // 解析 JSON
    let payload_json: serde_json::Value = match serde_json::from_str(&payload_str) {
        Ok(v) => v,
        Err(_) => return None,
    };

    // 提取 sub 字段
    payload_json["sub"]
        .as_str()
        .map(|s| s.split('|').nth(1).unwrap_or(s).to_string())
}

// 从 JWT token 中提取 time 字段
pub fn extract_time(token: &str) -> Option<DateTime<Local>> {
    // JWT token 由3部分组成，用 . 分隔
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    // 解码 payload (第二部分)
    let payload = match URL_SAFE_NO_PAD.decode(parts[1]) {
        Ok(decoded) => decoded,
        Err(_) => return None,
    };

    // 将 payload 转换为字符串
    let payload_str = match String::from_utf8(payload) {
        Ok(s) => s,
        Err(_) => return None,
    };

    // 解析 JSON
    let payload_json: serde_json::Value = match serde_json::from_str(&payload_str) {
        Ok(v) => v,
        Err(_) => return None,
    };

    // 提取时间戳并转换为本地时间
    payload_json["time"]
        .as_str()
        .and_then(|t| t.parse::<i64>().ok())
        .and_then(|timestamp| Local.timestamp_opt(timestamp, 0).single())
}
