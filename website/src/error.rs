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

/// An Axum middleware that routes error responses to a rendered error page.
///
/// If the response does not have a status code representing an error, it is returned as-is.
/// Otherwise, the response is inspected for a failure reason:
/// - If the content type of the response is "text/plain", the response body is used as the failure reason.
/// - If the status code of the response has a "canonical reason", this text is used as the failure reason. This
///   correlates to "Not found" for HTTP 404, and so on.
///
/// The failure reason is displayed on the error page, if it can be determined.
pub async fn to_error_page<B>(state: State, request: Request<B>, next: Next<B>) -> Response
where
    B: HttpBody,
{
    let mut response = next.run(request).await;

    // Successful responses need to be returned as-is.
    if !response.status().is_client_error() && !response.status().is_server_error() {
        return response;
    }

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| std::str::from_utf8(value.as_bytes()).ok())
        .unwrap_or("text/plain");

    let body_content = match content_type.contains("text/plain") {
        true => hyper::body::to_bytes(response.body_mut())
            .await
            .ok()
            .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_owned())
            .filter(|content| !content.is_empty()),
        false => None,
    };

    // If there was no explicit failure reason in the body, we should try and derive one from the status code.
    //
    // Unfortunately, not all status codes have associated canonical messages. If that's the case here, we'll just give
    // up. Maybe later we can do something funny, I dunno.
    let message = body_content.or_else(|| response.status().canonical_reason().map(sentence_case));
    let code = response.status().as_u16();
    let reason = match message {
        Some(message) => format!("Status code {}: {}", code, message),
        None => format!("Status code {} (no details provided...)", code),
    };

    let context = context!(
        "reason" => reason,
        "hide_navbar" => true,
    );

    Page::new("error", context)
        .render(state.engine())
        .into_http_result()
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
