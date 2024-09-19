use anyhow::{Context, Result};
use clap::Parser;
use time::format_description::well_known::Rfc3339;
use tracing_subscriber::EnvFilter;

use stock::cli::Args;
use stock::constants::SHUTDOWN_INTERVAL_SECS;
use stock::logic::main_loop;
use stock::types::MsgResponseType;
use stock_trading_cli_with_async_streams as stock;

/// The [`main`] function
///
/// We want to make it as small and as clean as possible,
/// so we delegate most of the tasks to the [`main_loop`] function.
///
/// This solution waits for tasks to fully finish by sleeping for some time.
/// It uses tokio.
/// This supports a fully-graceful shutdown, meaning all symbols will be fetched and processed
/// when a CTRL+C signal arrives.
// #[actix::main]
#[tokio::main]
async fn main() -> Result<MsgResponseType> {
    let args = Args::parse();

    // parse early so that neither main loop nor web app start
    // if date and time are not in the correct format
    time::OffsetDateTime::parse(&args.from, &Rfc3339)
        .context("The provided date or time format isn't correct.")?;

    // initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // spawn the main processing loop as a separate task
    tokio::spawn(async move { main_loop(args).await });

    // await the shutdown signal
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            tracing::info!(
                "\nCTRL+C received. Giving tasks some time ({} s) to finish...",
                SHUTDOWN_INTERVAL_SECS
            );
            tokio::time::sleep(tokio::time::Duration::from_secs(SHUTDOWN_INTERVAL_SECS)).await;
        }
        Err(err) => {
            // also shut down in case of an error
            tracing::error!("Unable to listen for the shutdown signal: {}", err);
        }
    }

    tracing::info!("Exiting now.");

    Ok(())
}
