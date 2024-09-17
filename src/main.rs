use anyhow::Result;
// use axum::Router;
// use axum::routing::get;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use stock::cli::Args;
use stock::constants::SHUTDOWN_INTERVAL_SECS;
// use stock::constants::{SHUTDOWN_INTERVAL_SECS, WEB_SERVER_ADDRESS};
// use stock::handlers::{get_desc, get_tail, root};
use stock::logic::main_loop;
use stock::types::MsgResponseType;
use stock_trading_cli_with_async_streams as stock;

// #[actix::main]
#[tokio::main]
async fn main() -> Result<MsgResponseType> {
    let args = Args::parse();

    // initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // This solution waits for tasks to fully finish by sleeping for some time.
    // It uses tokio.
    // This supports a fully-graceful shutdown, meaning all symbols will be fetched and processed
    // when a CTRL+C signal arrives.

    // spawn application as a separate task
    tokio::spawn(async move { main_loop(args).await });

    // // This doesn't fully support a graceful shutdown.
    // // This works with any async executor.
    // // It will not run an iteration of the main loop to completion upon receiving the CTRL+C signal.
    // // If the signal comes in the middle of data fetching and processing, only fetched symbols will be
    // // printed and saved to file. This only affects the last iteration of the main loop.
    // // The support still partially exists, as this will write data to stdout and to file for all
    // // symbols (tickers) that were processed before the CTRL+C signal came, so it is partly graceful.
    // //
    // // Writing to file is defined and started BEFORE the main loop begins!
    // // This is a difference between it and fetching and processing of data.
    // //
    // // If we don't care about the last iteration being potentially partial,
    // // this is good enough, it's simple, and it doesn't require tokio or tokio_util crates.
    // // Namely, we are experimenting with other async executors as well, although some of them
    // // might support tokio.
    // main_loop(args).await?;

    // TODO: Give these info/error messages to axum's signal (ctrl+c) handler - for graceful shutdown, and then remove this
    // Await the shutdown signal
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            tracing::info!(
                "\nCTRL+C received. Giving tasks some time ({} s) to finish...",
                SHUTDOWN_INTERVAL_SECS
            );
            tokio::time::sleep(tokio::time::Duration::from_secs(SHUTDOWN_INTERVAL_SECS)).await;
        }
        Err(err) => {
            // We also shut down in case of an error.
            tracing::error!("Unable to listen for the shutdown signal: {}", err);
        }
    }

    tracing::info!("Exiting now.");

    Ok(())
}
