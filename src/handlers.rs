//! Web-request handlers

use axum::{debug_handler, Json};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Html;
use serde::Serialize;

use crate::constants::TAIL_BUFFER_SIZE;
use crate::my_async_actors::CollectionActorHandle;

/// Our web app's state for keeping some variables
#[derive(Clone)]
pub struct WebAppState {
    /// The CLI argument `from`, so we don't have to pass it in tail response messages to the web app
    pub from: String,
    /// The single collection actor instance
    pub collection_handle: CollectionActorHandle,
}

/// List of last `n` batches, where each batch contains processed data for all S&P 500 symbols.
/// The batches are created at regular time intervals.
#[derive(Serialize)]
pub struct Tail {
    // TODO: Vec<Batch>
    tail: Vec<String>,
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

/// Fetches the last `n` iterations of the main loop, which occur at a fixed time interval,
/// and which include calculated performance indicators for all symbols.
///
/// If `n` is greater than the buffer size, we return the entire contents of the buffer,
/// whether it is full or not.
///
/// content-type: application/json
///
/// GET /tail/n
pub async fn get_tail(
    State(state): State<WebAppState>,
    Path(n): Path<usize>,
) -> (StatusCode, Json<Tail>) {
    let mut tail = last_n_batches(n).await;
    let t = tail
        .tail
        .iter()
        .map(|row| format!("{},{}", state.from, row));
    tail.tail = t.collect();

    (StatusCode::OK, Json(tail))
}

/// Describes the app
async fn description() -> Html<&'static str> {
    Html("<p>Stock Trading CLI with Async Streams</p>")
}

/// Fetches the last `n` batches of performance indicators for all symbols.
///
/// If `n` is greater than the buffer size, we return the entire contents of the buffer,
/// whether it is full or not.
async fn last_n_batches(n: usize) -> Tail {
    let n = n.clamp(0, TAIL_BUFFER_SIZE);
    // TODO
    let all: Vec<u8> = vec![1, 2, 3, 4, 5];
    let tail = all.iter().take(n).copied().map(|x| x.to_string()).collect();
    Tail { tail }
}
