use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use std::time::SystemTimeError;
use std::{fmt::Debug, num::ParseIntError};
use thiserror::Error;

pub type RtcResult<T, E=RtcError> = Result<T, E>;

#[derive(Error, Debug)]
pub enum WebsocketError {
    ConnectError(String),
    ListRoomError(String),
    DisconnectError(String),
    JoinRoomError(String),
    SendMessageError(String),
}
impl WebsocketError {
    fn as_str(&self) -> &str {
        match self {
            WebsocketError::ConnectError(e) => e,
            WebsocketError::ListRoomError(e) => e,
            WebsocketError::DisconnectError(e) => e,
            WebsocketError::JoinRoomError(e) => e,
            WebsocketError::SendMessageError(e) => e,
        }
    }
}

impl std::fmt::Display for WebsocketError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
#[derive(Error, Debug)]
pub enum RtcError
{
    #[error("Invalidate Jwt.")]
    InvalidateJWTError(String),
    #[error("{0}")]
    CustomerError(&'static str),
    #[error("JwtInvalidLength")]
    JwtInvalidLength(#[from] sha2::digest::InvalidLength),
    #[error("JwtVerifyFailed")]
    JwtVerifyFailed(#[from] jwt::Error),
    #[error("AnyHowError")]
    AnyHowError(#[from] anyhow::Error),
    #[error("ParseIntError")]
    ParseIntError(#[from] ParseIntError),
    #[error("SystemTimeError")]
    SystemTimeError(#[from] SystemTimeError),
    #[error("actix_web")]
    ActixWeb(#[from] actix_web::Error),
    #[error("websocket error: {0}")]
    WebsocketError(WebsocketError),
    #[error(transparent)]
    RecvError(RecvError),
}

impl ResponseError for RtcError
{
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).body(self.to_string())
    }

    fn status_code(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

use tokio::sync::{mpsc::error::SendError, oneshot::error::RecvError};

impl From<RecvError> for RtcError
{
    fn from(err: RecvError) -> RtcError {
        RtcError::RecvError(err)
    }
}

// impl<T> From<SendError<T>> for RtcError
// {
//     fn from(value: SendError<T>) -> Self {
//         Self::SendError(value)
//     }
// }
