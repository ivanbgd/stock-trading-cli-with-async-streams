use anyhow::Result;
use tokio_util::sync::CancellationToken;

use stock::logic::main_loop;
use stock::types::MsgResponseType;
use stock_trading_cli_with_async_streams as stock;

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
    // // If we don't care about the last iteration being potentially partial,
    // // this is good enough, it's simple, and it doesn't require tokio or tokio_util crates.
    // // Namely, we are experimenting with other async executors as well, although some of them
    // // might support tokio.
    // main_loop().await?;

    // The solution with cancellation tokens works in the same way as the above simple solution,
    // but it requires tokio AND tokio_util crates.
    let token = CancellationToken::new();
    let cloned_token = token.clone();

    let handle: tokio::task::JoinHandle<Result<MsgResponseType>> = tokio::spawn(async move {
        tokio::select! {
            _ = cloned_token.cancelled() => {
                println!("Shutting down...");
                Ok(())
            }
            loop_result = main_loop() => {
                loop_result
            }
        }
    });

    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                println!("\nCTRL+C received.");

                token.cancel();
            }
            Err(err) => {
                // We also shut down in case of an error.
                panic!("Unable to listen for the shutdown signal: {}", err);
            }
        }
    });

    let _ = handle.await?;

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
