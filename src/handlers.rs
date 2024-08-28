//! Request handlers

use axum::{debug_handler, Json};
// use anyhow::Result;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::Html;
use serde::Serialize;

use crate::constants::BUFFER_SIZE;

// TODO
/// List of last n signals
#[derive(Serialize)]
pub struct Tail {
    tail: Vec<u8>,
}

// TODO: return what CLI used to show as output, I guess...
/// todo: describe
///
/// content-type: application/json
///
/// GET /
#[debug_handler]
pub async fn root() -> (StatusCode, Json<&'static str>) {
    (StatusCode::OK, Json("TODO"))
}

/// Describes the app
///
/// content-type: text/html; charset=utf-8
///
/// GET /desc
pub async fn get_desc() -> Html<&'static str> {
    Html("<p>Stock Trading CLI with Async Streams</p>")
}

// TODO
/// todo: describe
///
/// content-type: application/json
///
/// GET /tail/n
pub async fn get_tail(Path(n): Path<usize>) -> Json<Tail> {
    let n = n.clamp(0, BUFFER_SIZE);
    let tail = last_n_signals(n).await;
    Json(tail)
}

// TODO
/// todo: describe
async fn last_n_signals(n: usize) -> Tail {
    let all: Vec<u8> = vec![1, 2, 3, 4, 5];
    let tail = all.iter().take(n).copied().collect();
    Tail { tail }
}
