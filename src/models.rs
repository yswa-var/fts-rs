use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    #[serde(rename = "dhanClientId")]
    pub dhan_client_id: String,

    #[serde(rename = "dhanClientName")]
    pub dhan_client_name: String,

    #[serde(rename = "dhanClientUcc")]
    pub dhan_client_ucc: String,

    #[serde(rename = "givenPowerOfAttorney")]
    pub given_power_of_attorney: bool,

    #[serde(rename = "accessToken")]
    pub access_token: String,

    #[serde(rename = "expiryTime")]
    pub expiry_time: String,
}
