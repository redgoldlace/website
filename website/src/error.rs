use crate::{context, page::Page, state::State};
use axum::{
    body::HttpBody,
    http::{header::CONTENT_TYPE, Request},
    middleware::Next,
    response::{IntoResponse, Response},
};
use figment::Error as FigmentError;
use hyper::{Error as HyperError, StatusCode};
use serde_json::Error as JsonError;
use std::{borrow::Cow, io::Error as IoError};
use syntect::LoadingError;
use tera::Error as TeraError;
use thiserror::Error;
use toml::de::Error as TomlError;

pub type HttpResult<T> = std::result::Result<T, HttpError>;
pub type Result<T> = std::result::Result<T, Error>;

/// An application-level error, as encountered during a HTTP request.
#[derive(Debug, Error)]
#[error("status code {status}: {cause}")]
pub struct HttpError {
    status: StatusCode,
    cause: Error,
}

impl HttpError {
    /// Create a new `HttpError` from the provided status code and underlying error,
    ///
    /// # Panics
    ///
    /// With debug assertions enabled, this function will panic if the provided status code does not represent an error.
    pub fn new(status: StatusCode, cause: Error) -> Self {
        debug_assert!(status.is_client_error() || status.is_server_error());

        Self { status, cause }
    }

    /// Create a new `HttpError` with a string message. The status code defaults to 500.
    pub fn msg(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, Error::msg(message))
    }

    /// Return a new `HttpError`, with the status code replaced by `status`.
    ///
    /// # Panics
    ///
    /// With debug assertions enabled, this function will panic if the provided status code does not represent an error.
    pub fn with_status(self, status: StatusCode) -> Self {
        debug_assert!(status.is_client_error() || status.is_server_error());

        Self { status, ..self }
    }

    /// Consume this `HttpError` and return a `(status, message)` tuple.
    ///
    /// The message is represented as a `Cow<'static, str>` to avoid allocating a `String` unnecessarily.
    pub fn into_response_parts(self) -> (StatusCode, Cow<'static, str>) {
        let message = match self.cause {
            Error::Custom(cause) => cause,
            error => Cow::from(error.to_string()),
        };

        (self.status, message)
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        // The error middleware will handle actually rendering the error page.
        self.into_response_parts().into_response()
    }
}

impl<E> From<E> for HttpError
where
    Error: From<E>,
{
    fn from(value: E) -> Self {
        HttpError::new(StatusCode::INTERNAL_SERVER_ERROR, Error::from(value))
    }
}

/// Helper trait to turn a `Result<T, Error>` into a `Result<T, HttpError>`.
pub trait IntoHttpResult<T> {
    fn into_http_result(self) -> HttpResult<T>;
}

impl<T, E> IntoHttpResult<T> for std::result::Result<T, E>
where
    Error: From<E>,
{
    fn into_http_result(self) -> HttpResult<T> {
        self.map_err(Error::from).map_err(Error::into_http_error)
    }
}

/// A generic application error.
#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Template(#[from] TeraError),
    #[error("{0}")]
    Io(#[from] IoError),
    #[error("{0}")]
    Toml(#[from] TomlError),
    #[error("{0}")]
    Json(#[from] JsonError),
    #[error("{0}")]
    Hyper(#[from] HyperError),
    #[error("{0}")]
    Syntax(#[from] LoadingError),
    #[error("{0}")]
    Config(#[from] FigmentError),
    #[error("{0}")]
    Custom(Cow<'static, str>),
}

impl Error {
    /// Create a new `Error` with a string message.
    pub fn msg(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Custom(message.into())
    }

    /// Upgrade this `Error` to a `HttpError`.
    ///
    /// A status code of 500 is used by default.
    pub fn into_http_error(self) -> HttpError {
        HttpError::new(StatusCode::INTERNAL_SERVER_ERROR, self)
    }
}

pub async fn to_error_page<B>(state: State, request: Request<B>, next: Next<B>) -> Response
where
    B: HttpBody,
{
    let mut response = next.run(request).await;
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| std::str::from_utf8(value.as_bytes()).ok())
        .unwrap_or("text/plain");

    if response.status().is_success() || !content_type.contains("text/plain") {
        return response;
    }

    let body_content = match hyper::body::to_bytes(response.body_mut()).await {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => return response, // How did this happen? Who knows.
    };

    // If there was no explicit message, we should try and derive one from the status.
    let message = match body_content.trim().is_empty() {
        true => response
            .status()
            .canonical_reason()
            .map(sentence_case)
            .map(Cow::from),
        false => Some(body_content.trim().into()),
    };

    // However not all status codes have associated canonical messages. If so, we'll just give up. Maybe later we can do
    // something funny, I dunno.
    let code = response.status().as_u16();
    let reason = match message {
        Some(message) => format!("Status code {}: {}", code, message),
        None => format!("Status code {} (no details provided...)", code),
    };

    Page::new("error", context!("reason" => reason))
        .render(state.engine())
        .map(|html| (response.status(), html))
        .into_response()
}

fn sentence_case(sentence: &str) -> String {
    match sentence.chars().next() {
        Some(first) => {
            let first_size = first.len_utf8();

            let mut result = sentence.to_owned();
            result[..first_size].make_ascii_uppercase();
            result[first_size..].make_ascii_lowercase();

            result
        }
        _ => String::new(),
    }
}
