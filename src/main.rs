use stock::logic::main_loop;
use stock_trading_cli_with_async_streams as stock;

#[tokio::main]
// async fn main() -> Result<MsgResponseType, actix::MailboxError> {
async fn main() {
    println!();

    // Spawn application as a separate task
    tokio::spawn(async move {
        let _ = main_loop().await;
        // main_loop().await?;
    });

    // Await the shutdown signal
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            println!("CTRL-C received. Shutting down...");
        }
        Err(err) => {
            // We also shut down in case of error.
            eprintln!("Unable to listen for the shutdown signal: {}", err);
        }
    }

    // Send the shutdown signal to application and wait for it to shut down
    // TODO: Implement broadcasting the shutdown signal

    // Ok(())
}
