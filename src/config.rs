use std::fmt;

pub struct Config {
    pub openai_api_key: String,
}

#[derive(Debug)]
pub enum ConfigError {
    MissingApiKey,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::MissingApiKey => {
                write!(f, "OPENAI_API_KEY environment variable is not set")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let openai_api_key =
            std::env::var("OPENAI_API_KEY").map_err(|_| ConfigError::MissingApiKey)?;
        Ok(Config { openai_api_key })
    }
}
