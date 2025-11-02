use crate::ShowMeErrors;
use jsonwebkey::JsonWebKey;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use openssl::base64;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::ops::Add;
use std::sync::Arc;

#[derive(Serialize, Debug)]
struct Payload {
    iss: String,
    sub: String,
    aud: String,
    exp: i64,
    jti: String,
}

impl Payload {
    fn update_exp(&mut self, exp: i64) {
        self.exp = exp;
    }
}

pub struct Token {
    pub token_string: Arc<String>,
    pub exp_date: i64,
    payload: Payload,
    key: EncodingKey,
    aud: String,
    pub dom: String
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    scope: String,
    token_type: String,
    expires_in: i64,
}

const THREE_MIN: i64 = 3 * 60;
const FIFTEEN_MIN: i64 = 899;

const CLIENT_ID: &str = "service-account";
const GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:jwt-bearer";
const SCOPE: &str = "fr:idm:* fr:am:*";
const AUD_PART: &str = "am/oauth2/access_token";


async fn token_exchange(payload: &Payload,aud: &str, key: &jsonwebtoken::EncodingKey) -> Result<String, ShowMeErrors> {
    let header = Header {
        alg: Algorithm::RS256,
        ..Default::default()
    };
    let test = encode(&header, payload, key)?;

    let client = reqwest::Client::new();
    let mut token_data = HashMap::new();
    token_data.insert("client_id".to_string(), CLIENT_ID.to_string());
    token_data.insert("grant_type".to_string(), GRANT_TYPE.to_string());
    token_data.insert("assertion".to_string(), test); // The JWT content
    token_data.insert("scope".to_string(), SCOPE.to_string());

    let token: TokenResponse = serde_json::from_slice(
        &*client
          .post(aud)
          .form(&token_data)
          .send()
          .await?
          .bytes()
          .await?,
    )?;

    Ok(token.access_token)
}


impl Token {
    fn new_or_update_exp(&self) -> (bool,i64) {
        let cur = self.exp_date;

        let now = chrono::Utc::now().timestamp();
        if now.min(THREE_MIN).le(&cur) {
            (true, now.add(FIFTEEN_MIN))
        } else {
            (false, cur)
        }
    }

    pub async fn get_usable_token(&mut self) -> Arc<String> {
        let (need_new_exp, exp) = self.new_or_update_exp();
        if need_new_exp {
            Arc::clone(&self.token_string)
        } else {
            self.payload.update_exp(exp);
            let token = token_exchange(&self.payload, &*self.aud, &self.key).await.unwrap_or(self.token_string.to_string());
            self.token_string = Arc::new(token);
            Arc::clone(&self.token_string)
        }
    }

    pub async fn new() -> Result<Self, ShowMeErrors> {
        let service_account_id =
            std::env::var("SA_ID").map_err(|_| ShowMeErrors::TokenDefault("SA_ID".to_string()))?;
        let dom =
            std::env::var("DOM").map_err(|_| ShowMeErrors::TokenDefault("DOM".to_string()))?;
        let aud = format!("{dom}/{AUD_PART}");
        let exp = chrono::Utc::now().timestamp().add(FIFTEEN_MIN);

        let mut buf = [0; 15];
        openssl::rand::rand_bytes(&mut buf)?;
        let jti = base64::encode_block(&buf);

        let payload = Payload {
            iss: service_account_id.clone(),
            sub: service_account_id,
            aud: aud.clone(),
            exp,
            jti,
        };

        let key =
            JsonWebKey::from_slice(fs::read_to_string(std::env::var("KEY_FILE")?)?)?.key.to_encoding_key();

        let header = Header {
            alg: Algorithm::RS256,
            ..Default::default()
        };
        let test = encode(&header, &payload, &key)?;

        let client = reqwest::Client::new();
        let mut token_data = HashMap::new();
        token_data.insert("client_id".to_string(), CLIENT_ID.to_string());
        token_data.insert("grant_type".to_string(), GRANT_TYPE.to_string());
        token_data.insert("assertion".to_string(), test); // The JWT content
        token_data.insert("scope".to_string(), SCOPE.to_string());

        let token: TokenResponse = serde_json::from_slice(
            &*client
                .post(&aud)
                .form(&token_data)
                .send()
                .await?
                .bytes()
                .await?,
        )?;

        Ok(Self {
            token_string: Arc::new(token.access_token),
            exp_date: exp,
            payload,
            key,
            aud,
            dom
        })
    }
}
