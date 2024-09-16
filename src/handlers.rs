//! Request handlers

use axum::{debug_handler, Json};
// use anyhow::Result;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::Html;
use serde::Serialize;

use crate::constants::TAIL_BUFFER_SIZE;

// TODO
/// List of last n signals
#[derive(Serialize)]
pub struct Tail {
    tail: Vec<u8>,
}

/// Describes the app
///
/// content-type: text/html; charset=utf-8
///
/// GET /
#[debug_handler]
pub async fn root() -> (StatusCode, Html<&'static str>) {
    (StatusCode::OK, description().await)
}

/// Describes the app
///
/// content-type: text/html; charset=utf-8
///
/// GET /desc
pub async fn get_desc() -> (StatusCode, Html<&'static str>) {
    (StatusCode::OK, description().await)
}

// TODO
/// todo: describe
/// Fetches the last `n` iterations of the main loop, which occur at a fixed time interval,
/// and which include calculated performance indicators for all symbols.
///
/// If `n` is greater than the buffer size, we return the entire contents of the buffer,
/// whether it is full or not.
///
/// content-type: application/json
///
/// GET /tail/n
pub async fn get_tail(Path(n): Path<usize>) -> Json<Tail> {
    let tail = last_n_iters(n).await;
    Json(tail)
}

/// Describes the app
async fn description() -> Html<&'static str> {
    Html("<p>Stock Trading CLI with Async Streams</p>")
}

// TODO
/// todo: describe
/// Fetches the last `n` iterations of the main loop, which occur at a fixed time interval,
/// and which include calculated performance indicators for all symbols.
///
/// If `n` is greater than the buffer size, we return the entire contents of the buffer,
/// whether it is full or not.
async fn last_n_iters(n: usize) -> Tail {
    let n = n.clamp(0, TAIL_BUFFER_SIZE);
    let all: Vec<u8> = vec![1, 2, 3, 4, 5];
    let tail = all.iter().take(n).copied().collect();
    Tail { tail }
}
