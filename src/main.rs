use std::env;

#[derive(Debug)]
struct Config {
    client_id: String,
    totp: String,
    dpin: String,
}

impl Config {
    fn from_env() -> Self {
        dotenvy::dotenv().ok();

        Self {
            client_id: env::var("CLIENT_ID").expect("CLIENT_ID is missing"),
            totp: env::var("TOTP_KEY").expect("TOTP_KEY is missing"),
            dpin: env::var("DPIN").expect("DPIN is missing"),
        }
    }
}

fn main() {
    let config = Config::from_env();

    println!("{:#?}", config);
}
