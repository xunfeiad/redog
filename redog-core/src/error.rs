use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("DNS resolution failed: {0}")]
    DnsResolution(String),

    #[error("Proxy handshake failed: {0}")]
    ProxyHandshake(String),

    #[error("Rule match error: {0}")]
    RuleMatch(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Timeout")]
    Timeout,

    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("{0}")]
    Other(String),
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::Other(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::Other(s.to_string())
    }
}
