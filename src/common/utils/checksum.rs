use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::Rng;
use sha2::{Digest, Sha256};

pub fn generate_hash() -> String {
    let random_bytes = rand::thread_rng().gen::<[u8; 32]>();
    let mut hasher = Sha256::new();
    hasher.update(random_bytes);
    hex::encode(hasher.finalize())
}

fn obfuscate_bytes(bytes: &mut [u8]) {
    let mut prev: u8 = 165;
    for (idx, byte) in bytes.iter_mut().enumerate() {
        let old_value = *byte;
        *byte = (old_value ^ prev).wrapping_add((idx % 256) as u8);
        prev = *byte;
    }
}

pub fn generate_checksum(device_id: &str, mac_addr: Option<&str>) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        / 1_000_000;

    let mut timestamp_bytes = vec![
        ((timestamp >> 40) & 255) as u8,
        ((timestamp >> 32) & 255) as u8,
        ((timestamp >> 24) & 255) as u8,
        ((timestamp >> 16) & 255) as u8,
        ((timestamp >> 8) & 255) as u8,
        (255 & timestamp) as u8,
    ];

    obfuscate_bytes(&mut timestamp_bytes);
    let encoded = BASE64.encode(&timestamp_bytes);

    match mac_addr {
        Some(mac) => format!("{}{}/{}", encoded, device_id, mac),
        None => format!("{}{}", encoded, device_id),
    }
}

pub fn validate_checksum(checksum: &str) -> bool {
    // 首先检查是否包含基本的 base64 编码部分和 hash 格式的 device_id
    let parts: Vec<&str> = checksum.split('/').collect();

    match parts.len() {
        // 没有 MAC 地址的情况
        1 => {
            // 检查是否包含 BASE64 编码的 timestamp (8字符) + 64字符的hash
            if checksum.len() != 72 {
                // 8 + 64 = 72
                return false;
            }

            // 验证 device_id hash 部分
            let device_hash = &checksum[8..];
            is_valid_hash(device_hash)
        }
        // 包含 MAC hash 的情况
        2 => {
            let first_part = parts[0];
            let mac_hash = parts[1];

            // MAC hash 必须是64字符的十六进制
            if !is_valid_hash(mac_hash) {
                return false;
            }

            // 递归验证第一部分
            validate_checksum(first_part)
        }
        _ => false,
    }
}

fn is_valid_hash(hash: &str) -> bool {
    // 检查长度是否为64
    if hash.len() != 64 {
        return false;
    }

    // 检查是否都是有效的十六进制字符
    hash.chars().all(|c| c.is_ascii_hexdigit())
}
