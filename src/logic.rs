//! The main loop
//!
//! The commented-out code is not dead code.
//!
//! Namely, this file contains several different implementations:
//! - async without Actors,
//! - my implementation of the Actor model,
//! - the Actix Actor framework.
//!
//! Naturally, only one implementation works at a time, so other need to be commented-out.
//!
//! They also use different imports, again, naturally - at least some are different.
//!
//! The purpose of this file, and the whole project for that matter, is to experiment
//! with different implementations and try out different things, so it was not meant to
//! look super-nice, but still care has been taken to some extent.

#![allow(unused_imports)]

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use actix::Actor;
use anyhow::{Context, Result};
use axum::Router;
use axum::routing::get;
use clap::Parser;
use rayon::prelude::*;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

// use crate::actix_async_actors::{handle_symbol_data, WriterActor};
use crate::cli::{Args, ImplementationVariant};
use crate::constants::{
    ACTOR_CHANNEL_CAPACITY, CHUNK_SIZE, CSV_HEADER, TICK_INTERVAL_SECS, WEB_SERVER_ADDRESS,
};
use crate::handlers::{get_desc, get_tail, get_tail_str, root, WebAppState};
use crate::my_async_actors::{
    ActorHandle, ActorMessage, CollectionActorHandle, UniversalActorHandle, WriterActorHandle,
};
use crate::types::MsgResponseType;

/// **The main loop**
///
/// This function does most of the work in our application.
///
/// It spawns a web application, and starts the main processing loop,
/// which fetches and processes symbol data, and also writes the
/// calculated performance indicators in a file, and additionally stores
/// the data in memory so that the web app can fetch it when a user
/// requires it.
///
/// This function supports several implementations. All but the currently
/// chosen one are commented-out in code.
///
/// Most implementations use the Actor model, and the main implementation
/// is based on it.
///
/// # Errors
/// - [time::error::Parse](https://docs.rs/time/0.3.36/time/error/enum.Parse.html)
pub async fn main_loop(args: Args) -> Result<MsgResponseType> {
    let from = OffsetDateTime::parse(&args.from, &Rfc3339)
        .context("The provided date or time format isn't correct.")?;
    let variant = args.variant;

    let symbols: Vec<String> = args.symbols.split(',').map(|s| s.to_string()).collect();
    static SYMBOLS: OnceLock<Vec<String>> = OnceLock::new();
    // let symbols = SYMBOLS.get_or_init(|| args.symbols.split(",").map(|s| s.to_string()).collect());
    let symbols = SYMBOLS.get_or_init(|| symbols);

    let chunks_of_symbols: Vec<&[String]> = match variant {
        ImplementationVariant::MyActorsNoRayon
        | ImplementationVariant::ActixActorsNoRayon
        | ImplementationVariant::NoActorsNoRayon => symbols.chunks(CHUNK_SIZE).collect(), // stdlib chunks

        ImplementationVariant::MyActorsRayon
        | ImplementationVariant::ActixActorsRayon
        | ImplementationVariant::NoActorsRayon => symbols.par_chunks(CHUNK_SIZE).collect(), // rayon parallel chunks
    };

    // used only in CollectionActor
    let nticks = symbols.len();

    // Use with my Actor implementation
    // Tested and it works with the integrated web application.
    let writer_handle = WriterActorHandle::new(nticks);
    let collection_handle = CollectionActorHandle::new(nticks);

    // // Use with Actix Actor implementation
    // // We need to ensure that we have one and only one `WriterActor` - a Singleton.
    // // This is because it writes to a file, and writing to a shared object,
    // // such as a file, needs to be synchronized, i.e., sequential.
    // // We generally don't use low-level synchronization primitives such as
    // // locks, mutexes, and similar when working with Actors.
    // // Actors have mailboxes and process messages that they receive one at a time,
    // // i.e., sequentially, and hence we can accomplish synchronization implicitly
    // // by using a single writer actor.
    // let writer_address = WriterActor::new().start();

    // // Use with async without Actors
    // let mut writer = start_writer()?;

    tracing::debug!("starting the web application");

    // build our web application with a state and with a route
    let state = WebAppState {
        from: args.from,
        collection_handle: collection_handle.clone(),
    };
    let app = Router::new()
        .route("/", get(root))
        .route("/desc", get(get_desc))
        .route("/tail/:n", get(get_tail))
        .route("/tailstr/:n", get(get_tail_str))
        .with_state(state);

    // run our web app with hyper
    // we need to spawn it as a separate tokio task so that we don't get blocked here
    let listener = tokio::net::TcpListener::bind(WEB_SERVER_ADDRESS).await?;
    tracing::info!("listening on {}", listener.local_addr()?);
    tokio::spawn(async move { axum::serve(listener, app).await });
    tracing::debug!("started the web application");

    tracing::debug!("starting the main loop");

    let mut interval = tokio::time::interval(Duration::from_secs(TICK_INTERVAL_SECS));

    loop {
        interval.tick().await;

        // We always want a fresh period end time, which is "now" in the UTC time zone.
        let to = OffsetDateTime::now_utc();

        // For standard output only, i.e., not for CSV
        println!("\n\n*** {} ***\n", to);

        // A simple way to output a CSV header
        println!("{}", CSV_HEADER);

        let start = Instant::now();

        //
        // WITH MY OWN IMPLEMENTATION OF ACTORS
        //

        // Without rayon. Not sequential. Multiple "`FetchActor`s" and "`ProcessorActor`s".
        // This is fast!
        //
        // This is considered the main, DEFAULT, implementation of the application.
        //
        // We start multiple instances of `Actor` - one per chunk of symbols,
        // and they will start the next `Actor` in the process - one each.
        // A single `ActorHandle` creates a single `Actor` instance and runs it on a new Tokio (asynchronous) task.
        //
        // Explicit concurrency with async/await paradigm: Run multiple instances of the same Future concurrently.
        // That's why it's fast - we spawn multiple tasks, i.e., multiple actors, concurrently, at the same time.
        // They'll also spawn multiple "`ProcessorActor`s" concurrently (at the same time).
        //
        // It's around 0.8 s on new computer with chunk size = 5; it wasn't measured on the old one.
        // It's less than 0.6 s on new computer with chunk size = 1!
        // It's around 1.4 s with CS = 10, and over 5 s with CS = 50.
        // Prints execution time after each chunk, which doesn't look super-nice, and that also
        // slows down execution a little, but at least we can measure the execution time,
        // which is important to us.
        //
        // Tested and it works with the integrated web application.
        for chunk in chunks_of_symbols.clone() {
            let actor_handle = UniversalActorHandle::new(nticks);
            let _ = actor_handle
                .send(ActorMessage::QuoteRequestsMsg {
                    symbols: chunk.into(),
                    from,
                    to,
                    writer_handle: writer_handle.clone(),
                    collection_handle: collection_handle.clone(),
                    start,
                })
                .await;
        }

        // // With rayon. Same speed as without rayon; fast (chunks or par_chunks doesn't make a difference).
        // // It's around 0.7 s on new computer with chunk size = 5; it wasn't measured on the old one.
        // // It's around 1.3 s with CS = 1, and around 1.3 s with CS = 10.
        // // Tested and it works with the integrated web application.
        // let queries: Vec<_> = chunks_of_symbols
        //     .par_iter()
        //     .map(|chunk| async {
        //         let actor_handle: UniversalActorHandle = ActorHandle::new(nticks);
        //         actor_handle
        //             .send(ActorMessage::QuoteRequestsMsg {
        //                 symbols: (*chunk).into(),
        //                 from,
        //                 to,
        //                 writer_handle: writer_handle.clone(),
        //                 collection_handle: collection_handle.clone(),
        //                 start,
        //             })
        //             .await
        //     })
        //     .collect();
        // let _ = futures::future::join_all(queries).await;

        //
        // WITH ACTIX ACTORS
        //

        // // Without rayon. Not sequential. Multiple `FetchActor`s and `ProcessorActor`s.
        // // Requires `#[actix::main]`.
        // // Around 0.8 seconds on new computer and 1.5 s on the old one, with chunk size = 5.
        // // Still around 0.8 s with CS = 1, and around 1.3-1.4 s with CS = 10.
        // // We start multiple `FetchActor`s - one per chunk of symbols,
        // // and they will start the next Actor in the process - one each.
        // // Explicit concurrency with async/await paradigm: Run multiple instances of the same Future concurrently.
        // // That's why it's fast - we spawn multiple tasks, i.e., multiple actors, concurrently, at the same time.
        // // They'll also spawn multiple `ProcessorActor`s concurrently (at the same time).
        // for chunk in chunks_of_symbols.clone() {
        //     let fetch_address = FetchActor.start();
        //
        //     let _ = fetch_address
        //         .send(QuoteRequestsMsg {
        //             chunk: chunk.into(),
        //             from,
        //             to,
        //             writer_address: writer_address.clone(),
        //             start,
        //         })
        //         .await?;
        // }

        // // With rayon. Not sequential. Multiple `FetchActor`s and `ProcessorActor`s.
        // // Requires `#[actix::main]`.
        // // Around 0.8 seconds on new computer and 1.5 s on the old one, with chunk size = 5.
        // // Around 0.9 s with CS = 1, and around 1.4 s with CS = 10.
        // // It is not much faster (if at all) than the above solution without rayon.
        // // Namely, execution time is not measured properly in this case, but it's roughly the same.
        // // Performance is the same when using regular (core) `chunks()` and `rayon`'s `par_chunks()`.
        // // We start multiple `FetchActor`s - one per chunk of symbols,
        // // and they will start the next Actor in the process - one each.
        // // Explicit concurrency with async/await paradigm: Run multiple instances of the same Future concurrently.
        // // That's why it's fast - we spawn multiple tasks, i.e., multiple actors, concurrently, at the same time.
        // // They'll also spawn multiple `ProcessorActor`s concurrently (at the same time).
        // let queries: Vec<_> = chunks_of_symbols
        //     .par_iter()
        //     .map(|chunk| async {
        //         FetchActor
        //             .start()
        //             .send(QuoteRequestsMsg {
        //                 chunk: (*chunk).into(),
        //                 from,
        //                 to,
        //                 writer_address: writer_address.clone(),
        //                 start,
        //             })
        //             .await
        //     })
        //     .collect();
        // let _ = futures::future::join_all(queries).await;

        //
        // ASYNC WITHOUT ACTORS
        //

        // // THE FASTEST SOLUTION - 0.7 s with chunk size of 5!
        // // This uses async fetching and processing of data.
        // //
        // // Tokio: 0.7-0.8 s (new computer); was 0.9 s on old computer - with chunk size = 5
        // // With CS = 1 it's ~1.3 s, and with CS = 10 it's ~1.3 s.
        // // Explicit concurrency with async/await paradigm:
        // // Run multiple instances of the same Future concurrently.
        // let mut handles = vec![];
        // for chunk in chunks_of_symbols.clone() {
        //     let handle = tokio::spawn(handle_symbol_data(chunk, from, to));
        //     handles.push(handle);
        // }
        // let rows = futures::future::join_all(handles).await;
        // let rows = rows.iter().map(|r| r.as_ref().unwrap()).collect::<Vec<_>>();
        // write_to_csv(&mut writer, rows, start)?;

        // // rayon: 0.8-0.9 s (new computer); was 1.0 s on old computer - with chunk size = 5
        // // With CS = 1 it's ~0.9 s, and with CS = 10 it's ~1.3 s.
        // let queries: Vec<_> = chunks_of_symbols
        //     .par_iter()
        //     .map(|chunk| handle_symbol_data(chunk, from, to))
        //     .collect();
        // let rows = futures::future::join_all(queries).await;
        // let rows = rows.iter().map(|r| r).collect::<Vec<_>>();
        // write_to_csv(&mut writer, rows, start)?;

        println!();
    }
}
