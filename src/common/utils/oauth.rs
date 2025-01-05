use anyhow::Result;
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl,
    TokenResponse, TokenUrl,
};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};

const OAUTH_AUTHORIZE_URL: &str = "https://connect.linux.do/oauth2/authorize";
const OAUTH_TOKEN_URL: &str = "https://connect.linux.do/oauth2/token";
const OAUTH_USER_INFO_URL: &str = "https://connect.linux.do/api/user";

#[derive(Serialize, Deserialize)]
pub struct ForumUser {
    pub id: i64,
    pub username: String,
    pub name: String,
    pub active: bool,
    pub trust_level: i32,
    pub silenced: bool,
}

pub struct ForumOAuth {
    oauth_client: BasicClient,
    http_client: Client,
}

impl ForumOAuth {
    pub fn new(client_id: String, client_secret: String, redirect_url: String) -> Result<Self> {
        let oauth_client = BasicClient::new(
            ClientId::new(client_id),
            Some(ClientSecret::new(client_secret)),
            AuthUrl::new(OAUTH_AUTHORIZE_URL.to_string())?,
            Some(TokenUrl::new(OAUTH_TOKEN_URL.to_string())?),
        )
        .set_redirect_uri(RedirectUrl::new(redirect_url)?);

        Ok(Self {
            oauth_client,
            http_client: Client::new(),
        })
    }

    pub fn get_authorize_url(&self) -> (Url, CsrfToken) {
        self.oauth_client
            .authorize_url(|| CsrfToken::new_random())
            .url()
    }

    pub async fn exchange_code_for_token(
        &self,
        code: &str,
        returned_state: &str,
        expected_state: &str,
    ) -> Result<String> {
        if returned_state != expected_state {
            return Err(anyhow::anyhow!("Invalid state parameter"));
        }

        let token = self
            .oauth_client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .request_async(oauth2::reqwest::async_http_client)
            .await?;

        Ok(token.access_token().secret().clone())
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
