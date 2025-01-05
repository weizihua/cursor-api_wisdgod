use crate::{
    app::constant::EMPTY_STRING,
    common::utils::parse_string_from_env,
};
use std::sync::LazyLock;

macro_rules! def_pub_static {
    // 基础版本：直接存储 String
    ($name:ident, $value:expr) => {
        pub static $name: LazyLock<String> = LazyLock::new(|| $value);
    };

    // 环境变量版本
    ($name:ident, env: $env_key:expr, default: $default:expr) => {
        pub static $name: LazyLock<String> =
            LazyLock::new(|| parse_string_from_env($env_key, $default).trim().to_string());
    };
}

// macro_rules! def_pub_static_getter {
//     ($name:ident) => {
//         paste::paste! {
//             pub fn [<get_ $name:lower>]() -> String {
//                 (*$name).clone()
//             }
//         }
//     };
// }

def_pub_static!(ROUTE_PREFIX, env: "ROUTE_PREFIX", default: "/v1");
def_pub_static!(PUBLIC_AUTH_TOKEN, env: "PUBLIC_AUTH_TOKEN", default: EMPTY_STRING);

pub static START_TIME: LazyLock<chrono::DateTime<chrono::Local>> =
    LazyLock::new(chrono::Local::now);

pub fn get_start_time() -> chrono::DateTime<chrono::Local> {
    *START_TIME
}

def_pub_static!(DEFAULT_INSTRUCTIONS, env: "DEFAULT_INSTRUCTIONS", default: "Respond in Chinese by default");

def_pub_static!(CURSOR_API2_HOST, env: "REVERSE_PROXY_HOST", default: "api2.cursor.sh");

pub static CURSOR_API2_BASE_URL: LazyLock<String> = LazyLock::new(|| {
    format!("https://{}/aiserver.v1.AiService/", *CURSOR_API2_HOST)
});

pub static OAUTH_CLIENT_ID: LazyLock<String> = LazyLock::new(|| {
    parse_string_from_env("OAUTH_CLIENT_ID", EMPTY_STRING).trim().to_string()
});

pub static OAUTH_CLIENT_SECRET: LazyLock<String> = LazyLock::new(|| {
    parse_string_from_env("OAUTH_CLIENT_SECRET", EMPTY_STRING).trim().to_string()
});

pub static OAUTH_REDIRECT_URI: LazyLock<String> = LazyLock::new(|| {
    parse_string_from_env("OAUTH_REDIRECT_URI", EMPTY_STRING).trim().to_string()
});

// pub static DEBUG: LazyLock<bool> = LazyLock::new(|| parse_bool_from_env("DEBUG", false));

// #[macro_export]
// macro_rules! debug_println {
//     ($($arg:tt)*) => {
//         if *crate::app::statics::DEBUG {
//             println!($($arg)*);
//         }
//     };
// }

def_pub_static!(ADMIN_AUTH_TOKEN, env: "ADMIN_AUTH_TOKEN", default: EMPTY_STRING);
