use std::time::{Duration, Instant};

use actix::Actor;
use actix_rt::System;
use async_std::prelude::StreamExt;
use async_std::stream;
use clap::Parser;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::actors::{MultiActor, QuoteRequest};
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
pub async fn main_loop() -> std::io::Result<()> {
    let args = Args::parse();
    let from = OffsetDateTime::parse(&args.from, &Rfc3339)
        .expect("The provided date or time format isn't correct.");

    let symbols: Vec<String> = args.symbols.split(",").map(|s| s.to_string()).collect();

    // let sys = System::new();
    // sys.block_on(async {
    //     let actor_address = MultiActor.start();
    // });
    // sys.run()?;

    let actor_address = MultiActor.start();

    let mut interval = stream::interval(Duration::from_secs(TICK_INTERVAL_SECS));

    while let Some(_) = interval.next().await {
        // We always want a fresh period end time.
        let to = OffsetDateTime::now_utc();

        // For standard output only, i.e., not for CSV
        println!("\n\n*** {} ***\n", OffsetDateTime::now_utc());

        // A simple way to output a CSV header
        println!("{}", CSV_HEADER);

        let start = Instant::now();

        for symbol in symbols.clone() {
            let _result = actor_address.send(QuoteRequest { symbol, from, to }).await;
        }

        // // Explicit concurrency with async-await paradigm:
        // // Run multiple instances of the same Future concurrently.
        // let queries: Vec<_> = symbols
        //     .iter()
        //     .map(|symbol| handle_symbol_data(symbol, from, to))
        //     .collect();
        // let _ = futures::future::join_all(queries).await;

        println!("\nTook {:.3?} to complete.", start.elapsed());
    }

    System::current().stop();

    Ok(())
}
