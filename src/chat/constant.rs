use std::sync::LazyLock;

use super::model::Model;

macro_rules! def_pub_const {
    ($name:ident, $value:expr) => {
        pub const $name: &'static str = $value;
    };
}
def_pub_const!(ERR_UNSUPPORTED_GIF, "不支持动态 GIF");
def_pub_const!(ERR_UNSUPPORTED_IMAGE_FORMAT, "不支持的图片格式，仅支持 PNG、JPEG、WEBP 和非动态 GIF");
def_pub_const!(ERR_NODATA, "No data");

pub const MODEL_OBJECT: &str = "model";
pub const CREATED: &i64 = &1706659200;

def_pub_const!(ANTHROPIC, "anthropic");
def_pub_const!(CURSOR, "cursor");
def_pub_const!(GOOGLE, "google");
def_pub_const!(OPENAI, "openai");

def_pub_const!(CLAUDE_3_5_SONNET, "claude-3.5-sonnet");
def_pub_const!(GPT_4, "gpt-4");
def_pub_const!(GPT_4O, "gpt-4o");
def_pub_const!(CLAUDE_3_OPUS, "claude-3-opus");
def_pub_const!(CURSOR_FAST, "cursor-fast");
def_pub_const!(CURSOR_SMALL, "cursor-small");
def_pub_const!(GPT_3_5_TURBO, "gpt-3.5-turbo");
def_pub_const!(GPT_4_TURBO_2024_04_09, "gpt-4-turbo-2024-04-09");
def_pub_const!(GPT_4O_128K, "gpt-4o-128k");
def_pub_const!(GEMINI_1_5_FLASH_500K, "gemini-1.5-flash-500k");
def_pub_const!(CLAUDE_3_HAIKU_200K, "claude-3-haiku-200k");
def_pub_const!(CLAUDE_3_5_SONNET_200K, "claude-3-5-sonnet-200k");
def_pub_const!(CLAUDE_3_5_SONNET_20241022, "claude-3-5-sonnet-20241022");
def_pub_const!(GPT_4O_MINI, "gpt-4o-mini");
def_pub_const!(O1_MINI, "o1-mini");
def_pub_const!(O1_PREVIEW, "o1-preview");
def_pub_const!(O1, "o1");
def_pub_const!(CLAUDE_3_5_HAIKU, "claude-3.5-haiku");
def_pub_const!(GEMINI_EXP_1206, "gemini-exp-1206");
def_pub_const!(
    GEMINI_2_0_FLASH_THINKING_EXP,
    "gemini-2.0-flash-thinking-exp"
);
def_pub_const!(GEMINI_2_0_FLASH_EXP, "gemini-2.0-flash-exp");

pub const AVAILABLE_MODELS: LazyLock<[Model; 21]> = LazyLock::new(|| [
    Model {
        id: CLAUDE_3_5_SONNET.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: ANTHROPIC,
    },
    Model {
        id: GPT_4.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: OPENAI,
    },
    Model {
        id: GPT_4O.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: OPENAI,
    },
    Model {
        id: CLAUDE_3_OPUS.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: ANTHROPIC,
    },
    Model {
        id: CURSOR_FAST.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: CURSOR,
    },
    Model {
        id: CURSOR_SMALL.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: CURSOR,
    },
    Model {
        id: GPT_3_5_TURBO.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: OPENAI,
    },
    Model {
        id: GPT_4_TURBO_2024_04_09.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: OPENAI,
    },
    Model {
        id: GPT_4O_128K.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: OPENAI,
    },
    Model {
        id: GEMINI_1_5_FLASH_500K.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: GOOGLE,
    },
    Model {
        id: CLAUDE_3_HAIKU_200K.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: ANTHROPIC,
    },
    Model {
        id: CLAUDE_3_5_SONNET_200K.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: ANTHROPIC,
    },
    Model {
        id: CLAUDE_3_5_SONNET_20241022.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: ANTHROPIC,
    },
    Model {
        id: GPT_4O_MINI.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: OPENAI,
    },
    Model {
        id: O1_MINI.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: OPENAI,
    },
    Model {
        id: O1_PREVIEW.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: OPENAI,
    },
    Model {
        id: O1.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: OPENAI,
    },
    Model {
        id: CLAUDE_3_5_HAIKU.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: ANTHROPIC,
    },
    Model {
        id: GEMINI_EXP_1206.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: GOOGLE,
    },
    Model {
        id: GEMINI_2_0_FLASH_THINKING_EXP.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: GOOGLE,
    },
    Model {
        id: GEMINI_2_0_FLASH_EXP.to_string(),
        created: CREATED,
        object: MODEL_OBJECT,
        owned_by: GOOGLE,
    },
]);

pub const USAGE_CHECK_MODELS: [&str; 11] = [
    CLAUDE_3_5_SONNET_20241022,
    CLAUDE_3_5_SONNET,
    GEMINI_EXP_1206,
    GPT_4,
    GPT_4_TURBO_2024_04_09,
    GPT_4O,
    CLAUDE_3_5_HAIKU,
    GPT_4O_128K,
    GEMINI_1_5_FLASH_500K,
    CLAUDE_3_HAIKU_200K,
    CLAUDE_3_5_SONNET_200K,
];
