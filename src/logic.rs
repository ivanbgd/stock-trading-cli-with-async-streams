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
use clap::Parser;
// use rayon::prelude::*;
use rayon::prelude::*;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

// use crate::actix_async_actors::{handle_symbol_data, WriterActor};
use crate::cli::Args;
use crate::constants::{CHUNK_SIZE, CSV_HEADER, TICK_INTERVAL_SECS};
use crate::my_async_actors::{ActorHandle, ActorMessage, UniversalActorHandle, WriterActorHandle};
// use crate::my_async_actors::{ActorHandle, ActorMessage, UniversalActorHandle, WriterActorHandle};
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

    // Use with async without Actors
    // let mut writer = start_writer()?;

    // Use with my Actor implementation
    let writer_handle = WriterActorHandle::new();

    // Use with Actix Actor implementation
    // We need to ensure that we have one and only one `WriterActor` - a Singleton.
    // This is because it writes to a file, and writing to a shared object,
    // such as a file, needs to be synchronized, i.e., sequential.
    // We generally don't use low-level synchronization primitives such as
    // locks, mutexes, and similar when working with Actors.
    // Actors have mailboxes and process messages that they receive one at a time,
    // i.e., sequentially, and hence we can accomplish synchronization implicitly
    // by using a single writer actor.
    // let writer_address = WriterActor::new().start();

    // let mut interval = stream::interval(Duration::from_secs(TICK_INTERVAL_SECS));
    let mut interval = tokio::time::interval(Duration::from_secs(TICK_INTERVAL_SECS));

    // while let Some(_) = interval.next().await {
    loop {
        // This doesn't exhibit the exact same behavior as when we handle `CTRL+C` in the function `main()`.
        // It is very similar, though, and can be considered the same.
        // Namely, if we handle CTRL+C in main() and a user sends the signal just before an iteration of this loop
        // begins, the iteration will be run.
        // If we instead handle CTRL+C here, in this loop, then if a user sends the signal when an iteration begins,
        // it will be run to completion, but it won't be run if the signal is sent just before an iteration begins.
        // All in all, in neither case will an iteration run only partly if it has already begun.
        // So, all iterations will run to completion if they start running.
        // Whether the last iteration will run at all depends on the exact moment when a user presses CTRL+C,
        // i.e., how long before a new iteration begins - whether it falls in the [`SHUTDOWN_INTERVAL_SECS`]
        // window.
        tokio::select! {
            _ = interval.tick() => {
                // We always want a fresh period end time, which is "now" in the UTC time zone.
                let to = OffsetDateTime::now_utc();

                // For standard output only, i.e., not for CSV
                println!("\n\n*** {} ***\n", to);

                // A simple way to output a CSV header
                println!("{}", CSV_HEADER);

                let start = Instant::now();

                //
                // NEW: ASYNC WITHOUT ACTORS, WITH WRITING TO FILE
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

                //
                // NEW WITH MY OWN IMPLEMENTATION OF ACTORS
                //

                // Without rayon. Not sequential. Multiple "`FetchActor`s" and "`ProcessorActor`s".
                // This is fast!
                //
                // This is considered the main implementation of the application.
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
                // for chunk in chunks_of_symbols.clone() {
                //     let actor_handle = UniversalActorHandle::new();
                //     let _ = actor_handle
                //         .send(ActorMessage::QuoteRequestsMsg {
                //             symbols: chunk.into(),
                //             from,
                //             to,
                //             writer_handle: writer_handle.clone(),
                //             start,
                //         })
                //         .await;
                // }

                // With rayon. Same speed as without rayon; fast (chunks or par_chunks doesn't make a difference).
                // It's around 0.7 s on new computer with chunk size = 5; it wasn't measured on the old one.
                // It's around 1.3 s with CS = 1, and around 1.3 s with CS = 10.
                let queries: Vec<_> = chunks_of_symbols
                    .par_iter()
                    .map(|chunk| async {
                        let actor_handle: UniversalActorHandle = ActorHandle::new();
                        actor_handle
                            .send(ActorMessage::QuoteRequestsMsg {
                                symbols: (*chunk).into(),
                                from,
                                to,
                                writer_handle: writer_handle.clone(),
                                start,
                            })
                            .await
                    })
                    .collect();
                let _ = futures::future::join_all(queries).await;

                //
                // NEW WITH ACTIX ACTORS
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

                println!();
            },
            // The break executes immediately if we don't sleep, and we consequently don't
            // process all symbols, even if a barrier (join) is used like we do here with the
            // rayon variant. So, this is not a fully-graceful shutdown.
            // Namely, we only join the first layer of Actors, which are fetching data,
            // but we are not joining the second layer of Actors, which process the data.
            // By the way, writing to file works because it's started before the loop.
            // So, this solution is partly-graceful.
            _ = tokio::signal::ctrl_c() => {
                println!("\nCTRL+C received.");

                // println!(
                //     "\nCTRL+C received. Giving tasks some time ({} s) to finish...",
                //     SHUTDOWN_INTERVAL_SECS
                // );
                // tokio::time::sleep(tokio::time::Duration::from_secs(SHUTDOWN_INTERVAL_SECS)).await;

                break Ok(());
            },
        }
    }

    // println!("OUT!!!");
    // stop_writer(writer); // Unreachable, but also unneeded if using Tokio's interval.

    // System::current().stop();

    // Ok(())
}
