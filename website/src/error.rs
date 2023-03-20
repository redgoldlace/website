// TODO: error type

use crate::{context, page::Page, state::State};
use axum::{
    body::HttpBody,
    http::{header::CONTENT_TYPE, Request},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::borrow::Cow;

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
