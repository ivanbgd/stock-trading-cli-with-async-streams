use stock::logic::main_loop;
use stock_trading_cli_with_async_streams as stock;

#[tokio::main]
// async fn main() -> Result<MsgResponseType, actix::MailboxError> {
async fn main() {
    println!();

    // let _ = main_loop().await;
    // main_loop().await?;

    tokio::spawn(async move {
        let _ = main_loop().await;
    });

    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            println!("CTRL-C received. Shutting down...");
        }
        Err(err) => {
            // We also shut down in case of error.
            eprintln!("Unable to listen for shutdown signal: {}", err);
        }
    }

    // Ok(())
}
