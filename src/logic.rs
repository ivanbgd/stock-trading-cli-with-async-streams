use std::io::{Error, ErrorKind};
use std::time::{Duration, Instant};

use actix::Actor;
use actix_rt::System;
use async_std::prelude::StreamExt;
use async_std::stream;
use clap::Parser;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use yahoo_finance_api as yahoo;

use crate::actors::MultiActor;
use crate::cli::Args;
use crate::constants::{TICK_INTERVAL_SECS, WINDOW_SIZE};
use crate::signals::{AsyncStockSignal, MaxPrice, MinPrice, PriceDifference, WindowedSMA};

///
/// Retrieve data from a data source and extract the closing prices
///
/// Errors during download are mapped onto `io::Errors` as `InvalidData`.
///
async fn fetch_closing_data(
    symbol: &str,
    beginning: OffsetDateTime,
    end: OffsetDateTime,
) -> std::io::Result<Vec<f64>> {
    let provider = yahoo::YahooConnector::new();

    let response = provider
        .get_quote_history(symbol, beginning, end)
        .await
        .map_err(|_| Error::from(ErrorKind::InvalidData))?;
    let mut quotes = response
        .quotes()
        .map_err(|_| Error::from(ErrorKind::InvalidData))?;
    if !quotes.is_empty() {
        quotes.sort_by_cached_key(|k| k.timestamp);
        Ok(quotes.iter().map(|q| q.adjclose).collect())
    } else {
        Ok(vec![])
    }
}

/// Convenience function that chains together the entire processing chain
async fn handle_symbol_data(
    symbol: &str,
    beginning: OffsetDateTime,
    end: OffsetDateTime,
) -> Option<Vec<f64>> {
    let closes = fetch_closing_data(symbol, beginning, end).await.ok()?;

    if !closes.is_empty() {
        let min = MinPrice {};
        let max = MaxPrice {};
        let price_diff = PriceDifference {};
        let n_window_sma = WindowedSMA {
            window_size: WINDOW_SIZE,
        };

        let period_min: f64 = min.calculate(&closes).await?;
        let period_max: f64 = max.calculate(&closes).await?;
        let last_price = *closes.last()?;
        let (_, pct_change) = price_diff.calculate(&closes).await?;
        let sma = n_window_sma.calculate(&closes).await?;

        // A simple way to output CSV data
        println!(
            "{},{},${:.2},{:.2}%,${:.2},${:.2},${:.2}",
            OffsetDateTime::format(beginning, &Rfc3339).expect("Couldn't format 'from'."),
            symbol,
            last_price,
            pct_change * 100.0,
            period_min,
            period_max,
            sma.last().unwrap_or(&0.0)
        );
    }

    Some(closes)
}

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
    let to = OffsetDateTime::now_utc();

    let symbols: Vec<&str> = args.symbols.split(",").collect();

    let my_actor = MultiActor {}.start();

    let mut interval = stream::interval(Duration::from_secs(TICK_INTERVAL_SECS));

    // A simple way to output a CSV header
    println!("period start,symbol,price,change %,min,max,30d avg");

    while let Some(_) = interval.next().await {
        // For standard output only, i.e., not for CSV
        println!("\n\n*** {} ***\n", OffsetDateTime::now_utc());

        let start = Instant::now();

        // let _result = my_actor.send(Msg(symbols.clone())).await;

        // Explicit concurrency with async-await paradigm:
        // Run multiple instances of the same Future concurrently.
        let queries: Vec<_> = symbols
            .iter()
            .map(|symbol| handle_symbol_data(symbol, from, to))
            .collect();
        let _ = futures::future::join_all(queries).await;

        println!("\nTook {:.3?} to complete.", start.elapsed());
    }

    System::current().stop();

    Ok(())
}
