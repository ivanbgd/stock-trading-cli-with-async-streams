use anyhow::Result;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use stock::logic::main_loop;
use stock::types::MsgResponseType;
use stock_trading_cli_with_async_streams as stock;
use stock_trading_cli_with_async_streams::constants::SHUTDOWN_INTERVAL_SECS;

// #[async_std::main]
// #[actix::main]
#[tokio::main]
async fn main() -> Result<MsgResponseType> {
    println!();

    // // This doesn't fully support a graceful shutdown.
    // // It will not run an iteration of the main loop to completion upon receiving the CTRL+C signal.
    // // If the signal comes in the middle of data fetching and processing, only fetched symbols will be
    // // printed and saved to file. This only affects the last iteration of the main loop.
    // // The support still partially exists, as this will write data to stdout and to file for all
    // // symbols (tickers) that were processed before the CTRL+C signal came, so it is partly graceful.
    // //
    // // Writing to file is defined and started BEFORE the main loop begins!
    // //
    // // If we don't care about the last iteration being potentially partial,
    // // this is good enough, it's simple, and it doesn't require tokio or tokio_util crates.
    // // Namely, we are experimenting with other async executors as well, although some of them
    // // might support tokio.
    // main_loop().await?;

    // This solution waits for tasks to fully finish by sleeping for some time.
    // This supports a fully-graceful shutdown, meaning, all symbols will be fetched and processed
    // when the CTRL+C signal arrives.
    // I consider this kind of hack and not a proper solution, but it works.
    let tracker = TaskTracker::new();
    let token = CancellationToken::new();

    let cloned_tracker = tracker.clone();
    let cloned_token = token.clone();

    tracker.spawn(async move {
        tokio::select! {
            _ = cloned_token.cancelled() => {
                println!("Shutting down now.");
                Ok(())
            }
            loop_result = main_loop() => {
                loop_result
            }
        }
    });

    // Spawn a background task that will send the shutdown signal.
    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                println!(
                    "\nCTRL+C received. Giving tasks some time ({} s) to finish...",
                    SHUTDOWN_INTERVAL_SECS
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(SHUTDOWN_INTERVAL_SECS)).await;

                cloned_tracker.close();
                token.cancel();
            }
            Err(err) => {
                // We also shut down in case of an error.
                panic!("Unable to listen for the shutdown signal: {}", err);
            }
        }
    });

    // Wait for all tasks to exit.
    tracker.wait().await;

    // tokio::select! {
    //     _ = tokio::signal::ctrl_c() => {
    //         println!("\nCTRL+C received. Shutting down...");
    //         return Ok(());
    //     },
    //     _ = main_loop() => {},
    // }

    // // Spawn application as a separate task
    // tokio::spawn(async move {
    //     let _ = main_loop().await;
    //     // main_loop().await?;
    // });

    // tokio::signal::ctrl_c().await?;
    // println!("\nCTRL+C received. Shutting down...");

    // // Await the shutdown signal
    // match tokio::signal::ctrl_c().await {
    //     Ok(()) => {
    //         println!("\nCTRL+C received. Shutting down...");
    //     }
    //     Err(err) => {
    //         // We also shut down in case of an error.
    //         eprintln!("Unable to listen for the shutdown signal: {}", err);
    //     }
    // }

    // Send the shutdown signal to application and wait for it to shut down
    // Implement broadcasting or sending the shutdown signal...

    Ok(())
}
