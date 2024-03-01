use std::time::{Duration, Instant};

use actix_rt::System;
use async_std::prelude::StreamExt;
use async_std::stream;
use clap::Parser;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::actors::handle_symbol_data;
use crate::cli::Args;
use crate::constants::{CHUNK_SIZE, CSV_HEADER, TICK_INTERVAL_SECS};

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
pub async fn main_loop() -> std::io::Result<()> {
    let args = Args::parse();
    let from = OffsetDateTime::parse(&args.from, &Rfc3339)
        .expect("The provided date or time format isn't correct.");

    // let symbols: Vec<String> = args.symbols.split(",").map(|s| s.to_string()).collect();
    let symbols: Vec<&str> = args.symbols.split(",").collect();
    let chunks_of_symbols = symbols.chunks(CHUNK_SIZE);

    // let actor_address = MultiActor.start();

    let mut interval = stream::interval(Duration::from_secs(TICK_INTERVAL_SECS));

    while let Some(_) = interval.next().await {
        // We always want a fresh period end time.
        let to = OffsetDateTime::now_utc();

        // For standard output only, i.e., not for CSV
        println!("\n\n*** {} ***\n", to);

        // A simple way to output a CSV header
        println!("{}", CSV_HEADER);

        let start = Instant::now();

        // for chunk in chunks_of_symbols.clone() {
        //     let chunk = chunk.to_vec();
        //     let _result = actor_address.send(QuoteRequest { chunk, from, to }).await;
        // }

        // THE FASTEST SOLUTION - around 2.4 s
        // Explicit concurrency with async/await paradigm:
        // Run multiple instances of the same Future concurrently.
        // This variant works with chunks of symbols instead of all symbols.
        // This implementation calls `handle_symbol_data()` that processes multiple symbol at a time,
        // but we don't see a speed improvement nevertheless.
        // Perhaps we are just limited by the data-fetching time from the Yahoo! Finance API.
        // We need to fetch data for around 500 symbols and the function `get_quote_history`
        // fetches data for one symbol (called "ticker") at a time. It is asynchronous,
        // but perhaps this is the best that we can do.
        // The time of 2.4 seconds is obtained with chunk size equal 1.
        // With chunk size equal 128, the time rises to over 13 seconds.
        let queries: Vec<_> = chunks_of_symbols
            .clone()
            .map(|chunk| handle_symbol_data(chunk, from, to))
            .collect();
        let _ = futures::future::join_all(queries).await; // Vec<()>

        // // THE FASTEST SOLUTION - around 2.5 s
        // // Explicit concurrency with async/await paradigm:
        // // Run multiple instances of the same Future concurrently.
        // let queries: Vec<_> = symbols
        //     .iter()
        //     .map(|symbol| handle_symbol_data(*symbol, from, to))
        //     .collect();
        // let _ = futures::future::join_all(queries).await; // Vec<Option<Vec<f64>>> (originally), or Vec<()> (now)

        println!("\nTook {:.3?} to complete.", start.elapsed());
    }

    System::current().stop();

    Ok(())
}
