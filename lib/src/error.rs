use thiserror::Error;

pub type Result<T> = std::result::Result<T, AgetError>;

#[derive(Error, Debug)]
pub enum AgetError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Config file parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("Extraction error: {0}")]
    Extraction(String),

    #[error("Cache error: {0}")]
    Cache(#[from] rusqlite::Error),
}

impl AgetError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn extraction(msg: impl Into<String>) -> Self {
        Self::Extraction(msg.into())
    }
}
