// TODO: error type

use crate::{context, page::Page, state::State};
use axum::{
    body::HttpBody,
    http::{header::CONTENT_TYPE, Request},
    middleware::Next,
    response::{IntoResponse, Response},
};

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

    let message = match hyper::body::to_bytes(response.body_mut()).await {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => return response, // How did this happen? Who knows.
    };

    let error_page = Page::new(
        "error",
        context!("reason" => format!("Status code {}: {}", response.status().as_u16(), message)),
    );

    error_page  
        .render(state.engine())
        .map(|html| (response.status(), html))
        .into_response()
}
