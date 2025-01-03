use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const OAUTH_AUTHORIZE_URL: &str = "https://connect.linux.do/oauth2/authorize";
const OAUTH_TOKEN_URL: &str = "https://connect.linux.do/oauth2/token";
const OAUTH_USER_INFO_URL: &str = "https://connect.linux.do/api/user";

#[derive(Debug, Serialize, Deserialize)]
pub struct ForumUser {
    pub id: i64,
    pub username: String,
    pub name: String,
    pub active: bool,
    pub trust_level: i32,
    pub silenced: bool,
}

pub struct ForumOAuth {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    http_client: Client,
}

impl ForumOAuth {
    pub fn new(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            client_id,
            client_secret,
            redirect_uri,
            http_client: Client::new(),
        }
    }

    pub fn get_authorize_url(&self, state: &str) -> String {
        format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&state={}",
            OAUTH_AUTHORIZE_URL,
            self.client_id,
            urlencoding::encode(&self.redirect_uri),
            state
        )
    }

    pub async fn exchange_code_for_token(&self, code: &str) -> Result<String> {
        let response = self
            .http_client
            .post(OAUTH_TOKEN_URL)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", &self.client_id),
                ("client_secret", &self.client_secret),
                ("redirect_uri", &self.redirect_uri),
            ])
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        Ok(response["access_token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No access token found"))?
            .to_string())
    }

    pub async fn get_user_info(&self, access_token: &str) -> Result<ForumUser> {
        let user = self
            .http_client
            .get(OAUTH_USER_INFO_URL)
            .bearer_auth(access_token)
            .send()
            .await?
            .json::<ForumUser>()
            .await?;

        Ok(user)
    }
}
