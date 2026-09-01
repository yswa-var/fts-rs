use std::env;

use totp_rs::{Algorithm, Builder, Secret};

#[derive(Debug)]
struct Config {
    client_id: String,
    totp_key: String,
    dpin: String,
}

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
}

fn main() {
    let config = Config::from_env();

    println!("{:#?}", config);

    let totp = config.current_totp();

    println!("TOTP: {}", totp)
}
