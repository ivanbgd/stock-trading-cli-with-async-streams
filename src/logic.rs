use std::time::{Duration, Instant};

// use actix::Actor;
// use actix_rt::System;
use async_std::prelude::StreamExt;
use async_std::stream;
use clap::Parser;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use xactor::{Broker, Service, Supervisor};

use crate::actors::{FileSink, QuoteRequest, StockDataDownloader, StockDataProcessor};
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
pub async fn main_loop() -> xactor::Result<()> {
    let args = Args::parse();
    let from = OffsetDateTime::parse(&args.from, &Rfc3339)
        .expect("The provided date or time format isn't correct.");

    let symbols: Vec<String> = args.symbols.split(",").map(|s| s.to_string()).collect();

    // let format = format_description::parse(
    //     // "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour sign:mandatory]:[offset_minute]:[offset_second]",
    //     "[year]-[month]-[day] [hour][minute][second]",
    // )?;

    // Start actors. Supervisors keep those actors alive.
    let _downloader = Supervisor::start(|| StockDataDownloader).await;
    let _processor = Supervisor::start(|| StockDataProcessor).await;
    let _sink = Supervisor::start(move || FileSink {
        // Create a unique file name every time
        // filename: format!(
        //     "{:?}.csv",
        //     OffsetDateTime::now_utc()
        //         .format(&format)
        //         .expect("Expected file name")
        // ),
        filename: "output.csv".to_string(),
        writer: None,
    })
    .await;

    let mut interval = stream::interval(Duration::from_secs(TICK_INTERVAL_SECS));

    'outer: while interval.next().await.is_some() {
        // We always want a fresh period end time.
        let to = OffsetDateTime::now_utc();

        // For standard output only, i.e., not for CSV
        println!("\n\n*** {} ***\n", to);

        // A simple way to output a CSV header
        println!("{}", CSV_HEADER);

        let start = Instant::now();

        for symbol in symbols.clone() {
            if let Err(e) =
                Broker::from_registry()
                    .await?
                    .publish(QuoteRequest { symbol, from, to })
            {
                eprint!("{}", e);
                break 'outer;
            }
        }

        println!("\nTook {:.3?} to complete.", start.elapsed());
    }

    // while let Some(_) = interval.next().await {
    //     // We always want a fresh period end time.
    //     let to = OffsetDateTime::now_utc();
    //
    //     // For standard output only, i.e., not for CSV
    //     println!("\n\n*** {} ***\n", to);
    //
    //     // A simple way to output a CSV header
    //     println!("{}", CSV_HEADER);
    //
    //     let start = Instant::now();
    //
    //     for symbol in symbols.clone() {
    //         let _result = actor_address.send(QuoteRequest { symbol, from, to }).await;
    //     }
    //
    //     // // Explicit concurrency with async-await paradigm:
    //     // // Run multiple instances of the same Future concurrently.
    //     // let queries: Vec<_> = symbols
    //     //     .iter()
    //     //     .map(|symbol| handle_symbol_data(symbol, from, to))
    //     //     .collect();
    //     // let _ = futures::future::join_all(queries).await;
    //
    //     println!("\nTook {:.3?} to complete.", start.elapsed());
    // }

    Ok(())
}
