use std::sync::OnceLock;
use std::time::{Duration, Instant};

use actix::Actor;
use actix_rt::System;
use async_std::prelude::StreamExt;
use async_std::stream;
use clap::Parser;
use rayon::prelude::*;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::actors::{FetchActor, QuoteRequestMsg};
use crate::cli::Args;
use crate::constants::{CSV_HEADER, TICK_INTERVAL_SECS};

/// **The main loop**
///
/// Implemented by using explicit concurrency with async-await paradigm.
///
/// Runs multiple instances of the same `Future` concurrently.
///
/// This uses the waiting time more efficiently.
///
/// This can increase the program's I/O throughput dramatically,
/// which may be needed to keep the strict schedule with an async
/// stream (that ticks every [`TICK_INTERVAL_SECS`] seconds), without
/// having to manage threads or data structures to retrieve results.
pub async fn main_loop() -> Result<(), actix::MailboxError> {
    let args = Args::parse();
    let from = OffsetDateTime::parse(&args.from, &Rfc3339)
        .expect("The provided date or time format isn't correct.");

    // let symbols: Vec<String> = args.symbols.split(",").map(|s| s.to_string()).collect();
    // let chunks_of_symbols: Vec<&[String]> = symbols.par_chunks(CHUNK_SIZE).collect();
    // let chunks_of_symbols: Vec<&[String]> = symbols.chunks(CHUNK_SIZE).collect();

    let symbols: Vec<String> = args.symbols.split(",").map(|s| s.to_string()).collect();
    static SYMBOLS: OnceLock<Vec<String>> = OnceLock::new();
    // // let symbols = SYMBOLS.get_or_init(|| args.symbols.split(",").map(|s| s.to_string()).collect());
    let symbols = SYMBOLS.get_or_init(|| symbols);
    // let chunks_of_symbols: Vec<&[String]> = symbols.chunks(CHUNK_SIZE).collect();
    // let chunks_of_symbols: Vec<&[String]> = symbols.par_chunks(CHUNK_SIZE).collect();

    // // let symbols: Vec<&str> = args.symbols.split(",").collect();
    // // let chunks_of_symbols = symbols.chunks(CHUNK_SIZE);

    // TODO: Spawn multiple `FetchActor`s. Perhaps move down into loop, or see another way - with ctx maybe?
    // let fetch_address = FetchActor.start();
    // let proc_writer_address = ProcessorWriterActor.start();
    // let actor_address = SyncArbiter::start(NUM_THREADS, || MultiActor); // Doesn't work (because of async handler).

    let mut interval = stream::interval(Duration::from_secs(TICK_INTERVAL_SECS));

    while let Some(_) = interval.next().await {
        // We always want a fresh period end time.
        let to = OffsetDateTime::now_utc();

        // For standard output only, i.e., not for CSV
        println!("\n\n*** {} ***\n", to);

        // A simple way to output a CSV header
        println!("{}", CSV_HEADER);

        let start = Instant::now();

        // NEW WITH ACTORS

        // // Without rayon. Not sequential. Multiple `FetchActor`s. 2.6 s
        //
        // // We start multiple `FetchActor`s - one per symbol, and they will
        // // start the next Actor in the process - one each.
        // for symbol in symbols.clone() {
        //     let fetch_address = FetchActor.start();
        //
        //     let _ = fetch_address
        //         .send(QuoteRequestMsg {
        //             symbol: symbol.to_string().clone(),
        //             from,
        //             to,
        //         })
        //         .await?;
        // }

        // With rayon. Not sequential. Multiple `FetchActor`s. ~2.5 s
        // It is not much faster (if at all) than the above solution without rayon.
        // Namely, execution time is not measured properly in this case.

        // We start multiple `FetchActor`s - one per symbol, and they will
        // start the next Actor in the process - one each.
        let queries: Vec<_> = symbols
            .par_iter()
            .map(|symbol| async {
                FetchActor
                    .start()
                    .send(QuoteRequestMsg {
                        symbol: symbol.to_string(),
                        from,
                        to,
                    })
                    .await
            })
            .collect();
        let _ = futures::future::join_all(queries).await;

        // OLD WITH ACTORS

        // for chunk in chunks_of_symbols.clone() {
        //     let chunk = chunk.to_vec();
        //     let _result = actor_address.send(QuoteRequest { chunk, from, to }).await;
        // }

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
        // let queries: Vec<_> = chunks_of_symbols
        //     .par_iter()
        //     .map(|chunk| handle_symbol_data(chunk, from, to))
        //     .collect();
        // let _ = futures::future::join_all(queries).await; // Vec<()>

        // // THE FASTEST SOLUTION - 1.2 s with chunk size of 5
        // // The `main()` function requires `#[actix::main]`.
        // // If we instead put `#[tokio::main]` it throws a panic.
        // let mut handles = vec![];
        // for chunk in chunks_of_symbols.clone() {
        //     let handle = tokio::spawn(handle_symbol_data(chunk, from, to));
        //     handles.push(handle);
        // }
        // let _ = futures::future::join_all(handles).await;

        println!("\nTook {:.3?} to complete.", start.elapsed());
    }

    System::current().stop();

    Ok(())
}
