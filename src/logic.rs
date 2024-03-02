use std::time::{Duration, Instant};

use actix::Actor;
use actix_rt::System;
use async_std::prelude::StreamExt;
use async_std::stream;
use clap::Parser;
use lazy_static::lazy_static;
use rayon::prelude::*;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::actors::{handle_symbol_data, MultiActor};
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
    // let chunks_of_symbols: Vec<&[String]> = symbols.par_chunks(CHUNK_SIZE).collect();
    // let chunks_of_symbols: Vec<&[String]> = symbols.chunks(CHUNK_SIZE).collect();

    // let symbols: Vec<String> = args.symbols.split(",").map(|s| s.to_string()).collect();
    // let symbols = Arc::new(Mutex::new(symbols));

    lazy_static! {
        static ref SYMBOLS: &'static str = "A,AAPL";
    }
    // const SYMBOLS: &'static str = "A,AAPL";
    let symbols: Vec<String> = SYMBOLS.split(",").map(|s| s.to_string()).collect(); // binding `symbols` declared here
    let chunks_of_symbols: Vec<&[String]> = symbols.par_chunks(CHUNK_SIZE).collect(); // error[E0597]: `symbols` does not live long enough

    // lazy_static! {
    //     static ref chunks_of_symbols: Vec<&'static [String]> =
    //         || { symbols }.par_chunks(CHUNK_SIZE).collect();
    // }

    // // let symbols: Vec<&str> = args.symbols.split(",").collect();
    // // let chunks_of_symbols = symbols.chunks(CHUNK_SIZE);

    let actor_address = MultiActor.start();
    // let actor_address = SyncArbiter::start(NUM_THREADS, || MultiActor);

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

        // let queries: Vec<_> = chunks_of_symbols
        //     .par_iter()
        //     .map(|chunk| async {
        //         actor_address
        //             .send(QuoteRequest {
        //                 chunk: chunk.to_vec(),
        //                 from,
        //                 to,
        //             })
        //             .await
        //     })
        //     .collect();
        // let _ = futures::future::join_all(queries).await;

        // // THE FASTEST SOLUTION - 1.2 s with chunk size of 5
        // // Explicit concurrency with async/await paradigm:
        // // Run multiple instances of the same Future concurrently.
        // let queries: Vec<_> = chunks_of_symbols
        //     .par_iter()
        //     .map(|chunk| handle_symbol_data(chunk, from, to))
        //     .collect();
        // let _ = futures::future::join_all(queries).await; // Vec<()>

        let mut handles = vec![];
        // let symbols = symbols.clone();
        for chunk in chunks_of_symbols.clone() {
            let handle = std::thread::spawn(move || async move {
                handle_symbol_data(chunk, from, to).await;
            });
            handles.push(handle);
        }
        // handle.join().unwrap().await;
        // let _ = futures::future::join_all(handles).await;
        for handle in handles {
            handle.join().unwrap().await;
        }

        println!("\nTook {:.3?} to complete.", start.elapsed());
    }

    System::current().stop();

    Ok(())
}
