use crate::models::AuthResponse;

use chrono::{NaiveDateTime, Utc};

use reqwest::Client;
use std::{env, fs, path::Path};
use totp_rs::{Algorithm, Builder, Secret};
use tracing::{error, info, warn};

#[derive(Debug)]
struct Config {
    client_id: String,
    totp_key: String,
    dpin: String,
}

const AUTH_FILE: &str = "auth.json";

impl Config {
    fn from_env() -> Self {
        dotenvy::dotenv().ok();

        Self {
            client_id: env::var("CLIENT_ID").expect("CLIENT_ID is missing"),
            totp_key: env::var("TOTP_KEY").expect("TOTP_KEY is missing"),
            dpin: env::var("DPIN").expect("DPIN is missing"),
        }
    }

    fn current_totp(&self) -> String {
        let secret = Secret::try_from_base32(&self.totp_key).expect("Invalid TOTP secret");

        let totp = Builder::new()
            .with_secret(secret)
            .with_algorithm(Algorithm::SHA1)
            .build()
            .expect("Failed to build TOTP");

        totp.generate_current().to_string()
    }

    async fn generate_access_token(&self) -> Result<AuthResponse, Box<dyn std::error::Error>> {
        let current_totp = self.current_totp();

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        let response = client
            .post("https://auth.dhan.co/app/generateAccessToken")
            .query(&[
                ("dhanClientId", &self.client_id),
                ("pin", &self.dpin),
                ("totp", &current_totp),
            ])
            .header("Accept", "application/json")
            .send()
            .await?;

        let status = response.status();
        let data = response.text().await?;

        if status.is_success() {
            let auth: AuthResponse = serde_json::from_str(&data)?;
            Ok(auth)
        } else {
            Err(format!("Dhan API returned {}: {}", status, data).into())
        }
    }

    async fn get_access_token(&self) -> Result<AuthResponse, Box<dyn std::error::Error>> {
        // 1. Try cached auth.json
        if Path::new(AUTH_FILE).exists() {
            let contents = fs::read_to_string(AUTH_FILE)?;

            let auth: AuthResponse = serde_json::from_str(&contents)?;

            let expiry_time =
                NaiveDateTime::parse_from_str(&auth.expiry_time, "%Y-%m-%dT%H:%M:%S%.f")?;

            let expiry_time = expiry_time.and_utc();

            if Utc::now() < expiry_time {
                info!("Using cached Dhan access token");

                return Ok(auth);
            }

            warn!("Cached Dhan access token has expired");
        }

        // 2. No cache -> generate new token
        info!("Generating new Dhan access token");

        let auth = self.generate_access_token().await?;

        // 3. Persist auth response
        let json = serde_json::to_string_pretty(&auth)?;

        fs::write(AUTH_FILE, json)?;

        Ok(auth)
    }
}

pub async fn get_access_token() -> Result<(String, String), Box<dyn std::error::Error>> {
    Config::from_env()
        .get_access_token()
        .await
        .map(|auth| {
            info!("Successfully authenticated with Dhan");
            (auth.access_token, auth.dhan_client_id)
        })
        .map_err(|error| {
            error!("Authentication failed: {}", error);
            error
        })
}
