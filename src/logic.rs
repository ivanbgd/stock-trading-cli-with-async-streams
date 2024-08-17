// use std::error::Error;
// use std::sync::OnceLock;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
// use async_std::stream::{self, StreamExt};
use clap::Parser;
// use rayon::prelude::*;
// use rayon::prelude::*;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

// use crate::actix_async_actors::{handle_symbol_data, WriterActor};
// use crate::my_async_actors::{ActorHandle, ActorMessage, UniversalActorHandle, WriterActorHandle};
use crate::cli::Args;
use crate::constants::{CHUNK_SIZE, CSV_HEADER, TICK_INTERVAL_SECS};
use crate::process::{handle_symbol_data, start_writer, write_to_csv};
use crate::types::MsgResponseType;

/// **The main loop**
///
/// Implemented by using classical multithreading for concurrency.
///
/// Runs multiple instances of the same task concurrently.
///
/// To be more precise, this is a parallel implementation.
///
/// Async code is used for intervals.
///
/// Async code is also used for fetching and processing of data.
///
/// # Errors
/// - [time::error::Parse](https://docs.rs/time/0.3.36/time/error/enum.Parse.html)
// pub async fn main_loop() -> Result<MsgResponseType, actix::MailboxError> {
pub async fn main_loop() -> Result<MsgResponseType> {
    let args = Args::parse();
    let from = OffsetDateTime::parse(&args.from, &Rfc3339)
        .context("The provided date or time format isn't correct.")?;

    // let symbols: Vec<String> = args.symbols.split(',').map(|s| s.to_string()).collect();
    // // If we use rayon and its `par_iter()`, it doesn't make a difference in our case whether we use
    // // stdlib chunks or rayon chunks.
    // // let chunks_of_symbols: Vec<&[String]> = symbols.chunks(CHUNK_SIZE).collect(); // stdlib chunks
    // let chunks_of_symbols: Vec<&[String]> = symbols.par_chunks(CHUNK_SIZE).collect(); // rayon parallel chunks

    // This is required only if using Tokio.
    let symbols: Vec<String> = args.symbols.split(',').map(|s| s.to_string()).collect();
    static SYMBOLS: OnceLock<Vec<String>> = OnceLock::new();
    // // let symbols = SYMBOLS.get_or_init(|| args.symbols.split(",").map(|s| s.to_string()).collect());
    let symbols = SYMBOLS.get_or_init(|| symbols);
    // let chunks_of_symbols: Vec<&[String]> = symbols.par_chunks(CHUNK_SIZE).collect(); // rayon parallel chunks
    let chunks_of_symbols: Vec<&[String]> = symbols.chunks(CHUNK_SIZE).collect(); // stdlib chunks

    // // We need to ensure that we have one and only one `WriterActor` - a singleton.
    // // This is because it writes to a file, and writing to a shared object,
    // // such as a file, needs to be synchronized, i.e., sequential.
    // // We generally don't use low-level synchronization primitives such as
    // // locks, mutexes, and similar when working with Actors.
    // // Actors have mailboxes and process messages that they receive one at a time,
    // // i.e., sequentially, and hence we can accomplish synchronization implicitly
    // // by using a single writer actor.
    // let writer_address = WriterActor::new().start();

    // let writer_handle = WriterActorHandle::new();

    let mut writer = start_writer()?;

    // let mut interval = stream::interval(Duration::from_secs(TICK_INTERVAL_SECS));
    let mut interval = tokio::time::interval(Duration::from_secs(TICK_INTERVAL_SECS));

    // for _ in 0..1 { // TODO: remove the FOR line
    // while let Some(_) = interval.next().await {
    loop {
        interval.tick().await;

        // We always want a fresh period end time, which is "now" in the UTC time zone.
        let to = OffsetDateTime::now_utc();

        // For standard output only, i.e., not for CSV
        println!("\n\n*** {} ***\n", to);

        // A simple way to output a CSV header
        println!("{}", CSV_HEADER);

        let start = Instant::now();

        // NEW: SYNC (BLOCKING) WITHOUT ACTORS, WITH WRITING TO FILE

        // THE FASTEST SOLUTION - 0.9 s with chunk size of 5!
        // This uses async fetching and processing of data.

        // // rayon: 1.0 s
        // let queries: Vec<_> = chunks_of_symbols
        //     .par_iter()
        //     .map(|chunk| handle_symbol_data(chunk, from, to))
        //     .collect();
        // let rows = futures::future::join_all(queries).await;
        // write_to_csv(&mut writer, rows);

        // Tokio: 0.7-0.8 s (new computer); was 0.9 s
        let mut handles = vec![];
        for chunk in chunks_of_symbols.clone() {
            let handle = tokio::spawn(handle_symbol_data(chunk, from, to));
            handles.push(handle);
        }
        let rows = futures::future::join_all(handles).await;
        let rows = rows.iter().map(|r| r.as_ref().unwrap()).collect::<Vec<_>>();
        write_to_csv(&mut writer, rows)?;

        // NEW WITH MY OWN IMPLEMENTATION OF ACTORS

        // Without rayon. Not sequential. Multiple "`FetchActor`s" and "`ProcessorActor`s".
        // This is fast! Possibly even below a second.

        // // We start multiple instances of `Actor` - one per chunk of symbols,
        // // and they will start the next `Actor` in the process - one each.
        // // A single `ActorHandle` creates a single `Actor` instance and runs it on a new Tokio (asynchronous) task.
        // //
        // // Explicit concurrency with async/await paradigm: Run multiple instances of the same Future concurrently.
        // // That's why it's fast - we spawn multiple tasks, i.e., multiple actors, concurrently, at the same time.
        // // They'll also spawn multiple "`ProcessorActor`s" concurrently (at the same time).
        // for chunk in chunks_of_symbols.clone() {
        //     let actor_handle = UniversalActorHandle::new();
        //     let _ = actor_handle
        //         .send(ActorMessage::QuoteRequestsMsg {
        //             symbols: chunk.into(),
        //             from,
        //             to,
        //             writer_handle: writer_handle.clone(),
        //         })
        //         .await;
        // }

        // // With rayon. Same speed as without rayon; fast (chunks or par_chunks don't make a difference).
        //
        // let queries: Vec<_> = chunks_of_symbols
        //     .par_iter()
        //     .map(|chunk| async {
        //         ActorHandle::new()
        //             .send(ActorMessage::QuoteRequestsMsg {
        //                 symbols: (*chunk).into(),
        //                 from,
        //                 to,
        //                 // writer_address: writer_address.clone(),
        //             })
        //             .await
        //     })
        //     .collect();
        // let _ = futures::future::join_all(queries).await;

        // NEW WITH ACTORS

        // Without rayon. Not sequential. Multiple `FetchActor`s and `ProcessorActor`s. Possibly around 1.5 seconds.

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
        //         })
        //         .await?;
        // }

        // // With rayon. Not sequential. Multiple `FetchActor`s and `ProcessorActor`s. Possibly around 1.5 seconds.
        // // It is not much faster (if at all) than the above solution without rayon.
        // // Namely, execution time is not measured properly in this case, but it's roughly the same.
        // // Performance is the same when using regular (core) `chunks()` and `rayon`'s `par_chunks()`.
        //
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
        //             })
        //             .await
        //     })
        //     .collect();
        // let _ = futures::future::join_all(queries).await;

        // OLD WITH ACTORS

        // // This is not using rayon.
        // for chunk in chunks_of_symbols.clone() {
        //     let chunk = chunk.to_vec();
        //     let _result = actor_address.send(QuoteRequest { chunk, from, to }).await;
        // }

        // // This is using rayon.
        // let queries: Vec<_> = chunks_of_symbols
        //     .par_iter()
        //     .map(|chunk| async {
        //         actor_address
        //             .send(QuoteRequestMsg {
        //                 chunk: chunk.to_vec(),
        //                 from,
        //                 to,
        //             })
        //             .await
        //     })
        //     .collect();
        // let _ = futures::future::join_all(queries).await;

        // OLD WITHOUT ACTORS

        // // THE FASTEST SOLUTION - 1.2 s with chunk size of 5
        // // Explicit concurrency with async/await paradigm:
        // // Run multiple instances of the same Future concurrently.
        // // This is using rayon.
        // let queries: Vec<_> = chunks_of_symbols
        //     .par_iter()
        //     .map(|chunk| handle_symbol_data(chunk, from, to))
        //     .collect();
        // let _ = futures::future::join_all(queries).await; // Vec<()>

        // // THE FASTEST SOLUTION - 1.2 s with chunk size of 5
        // // The `main()` function requires `#[actix::main]`.
        // // If we instead put `#[tokio::main]` it throws a panic.
        // // This is using Tokio.
        // let mut handles = vec![];
        // for chunk in chunks_of_symbols.clone() {
        //     let handle = tokio::spawn(handle_symbol_data(chunk, from, to));
        //     handles.push(handle);
        // }
        // let _ = futures::future::join_all(handles).await;

        println!("\nTook {:.3?} to complete.", start.elapsed());
    }

    // println!("OUT!!!");
    // stop_writer(writer); // Unreachable, but also unneeded if using Tokio's interval.

    // System::current().stop();

    // Ok(())
}
