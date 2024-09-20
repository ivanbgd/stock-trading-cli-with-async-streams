//! Web-request handlers

use axum::{debug_handler, Json};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Html;
use serde::Serialize;
use tokio::sync::mpsc;

use crate::constants::{ACTOR_CHANNEL_CAPACITY, TAIL_BUFFER_SIZE};
use crate::my_async_actors::{ActorHandle, CollectionActorHandle, CollectionActorMsg};
use crate::types::{TailResponse, TailResponseString};

/// Our web app's state for keeping some variables
#[derive(Clone)]
pub struct WebAppState {
    /// The CLI argument `from`, so we don't have to pass it in tail response messages to the web app
    pub from: String,
    /// The single collection actor instance
    pub collection_handle: CollectionActorHandle,
}

/// An array of the last `n` fully-assembled batches,
/// where each batch contains processed data for all S&P 500 symbols.
///
/// The batches are created at regular time intervals.
///
// ///Not strictly needed, because this is just a wrapper type, but we kept it for completeness.
#[derive(Serialize)]
// pub struct Tail(TailResponse);
pub struct Tail {
    from: String,
    tail: TailResponse,
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
/// Works with [`crate::my_async_actors::PerformanceIndicatorsRow`]s.
///
/// content-type: application/json
///
/// GET /tail/n
pub async fn get_tail(
    State(state): State<WebAppState>,
    Path(n): Path<usize>,
) -> (StatusCode, Json<Tail>) {
    // limit n to buffer capacity
    let n = n.clamp(0, TAIL_BUFFER_SIZE);

    // create channel for sending the collection actor a tail request message
    let (sender, mut receiver) = mpsc::channel(ACTOR_CHANNEL_CAPACITY);

    // our web application acts like an actor here, sending the collection actor a message
    //
    // we use the actor's send method for sending it the message, which is the only way
    // to send an actor a message anyway
    //
    // in the message, we give it the sending half of the channel, and the requested number of batches, n
    let _ = state
        .collection_handle
        .send(CollectionActorMsg::TailRequest { sender, n })
        .await;

    // then we wait (block) for response from the collection actor, which we receive
    // at the receiving half of the channel
    if let Some(tail) = receiver.recv().await {
        // we add the *from* field only at the beginning of the batch, and to at the
        // beginning of each row, but this should be enough
        // (StatusCode::OK, Json(Tail(tail)))
        (
            StatusCode::OK,
            Json(Tail {
                from: state.from,
                tail,
            }),
        )
    } else {
        // (StatusCode::INTERNAL_SERVER_ERROR, Json(Tail(vec![])))
        (
            StatusCode::OK,
            Json(Tail {
                from: state.from,
                tail: vec![],
            }),
        )
    }

    // // let mut response = TailResponse::new();
    // let mut batches = Vec::new();
    // for batch in tail {
    //     let mut new_batch = Vec::new();
    //     for row in batch {
    //         let new_row = format!("{},{}", state.from, row);
    //         new_batch.push(new_row);
    //     }
    //     batches.push(new_batch);
    // }
    //
    // (StatusCode::OK, Json(batches))

    // let t = tail
    //     .iter()
    //     // .flatten()
    //     .map(|row| format!("{},{:?}", state.from, row))
    //     .collect();
    // let tail = Tail(t);

    // (StatusCode::OK, Json(tail))
    // (StatusCode::OK, Json(Tail(tail)))
}

/// Fetches the last `n` iterations of the main loop, which occur at a fixed time interval,
/// and which include calculated performance indicators for all symbols.
///
/// If `n` is greater than the buffer size, we return the entire contents of the buffer,
/// whether it is full or not.
///
/// Works with [`String`]s instead of [`crate::my_async_actors::PerformanceIndicatorsRow`]s.
///
/// This output looks like the CLI output (`stdout` or tracing output), which is also the same
/// as the CSV file format that we write.
///
/// content-type: application/json
///
/// GET /tailstr/n
pub async fn get_tail_str(
    State(state): State<WebAppState>,
    Path(n): Path<usize>,
) -> (StatusCode, Json<TailResponseString>) {
    // limit n to buffer capacity
    let n = n.clamp(0, TAIL_BUFFER_SIZE);

    // create channel for sending the collection actor a tail request message
    let (sender, mut receiver) = mpsc::channel(ACTOR_CHANNEL_CAPACITY);

    // our web application acts like an actor here, sending the collection actor a message
    //
    // we use the actor's send method for sending it the message, which is the only way
    // to send an actor a message anyway
    //
    // in the message, we give it the sending half of the channel, and the requested number of batches, n
    let _ = state
        .collection_handle
        .send(CollectionActorMsg::TailRequest { sender, n })
        .await;

    // then we wait (block) for response from the collection actor, which we receive
    // at the receiving half of the channel
    if let Some(tail) = receiver.recv().await {
        // we now add the *from* field at the beginning of each row that goes to output
        //
        // since we use the same message type as in [`get_tail`], the same message handler is used inside
        // the collection actor, and it returns [`TailResponse`], which is the above `tail` variable
        //
        // we (currently) don't have an iterator over [`TailResponse`], so we need to use the nested loops
        let mut batches = Vec::new();
        for batch in tail {
            let mut new_batch = Vec::new();
            for row in batch {
                let new_row = format!("{},{}", state.from, row);
                new_batch.push(new_row);
            }
            batches.push(new_batch);
        }
        (StatusCode::OK, Json(batches))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(vec![]))
    }
}

/// Describes the app
async fn description() -> Html<&'static str> {
    Html("<p>Stock Trading CLI with Async Streams</p>")
}
