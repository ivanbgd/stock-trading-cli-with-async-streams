use stock::logic::main_loop;
use stock_trading_cli_with_async_streams as stock;

#[async_std::main]
// #[tokio::main]
// async fn main() -> Result<MsgResponseType, actix::MailboxError> {
async fn main() {
    println!();

    // main_loop().await;

    let (tx, rx) = tokio::sync::mpsc::channel(1);
    let tx2 = tx.clone();

    // Spawn application as a separate task
    tokio::spawn(async move {
        let _ = main_loop(rx).await;
        // let _ = tx.clone().send(false).await;
        // main_loop().await?;
    });

    // Await the shutdown signal
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            println!("CTRL+C received. Shutting down..."); // This part works. We'll stop the program whatever we send below.
            let _ = tx2.send(true).await; // This part works (whatever we send, true or false, it will be evaluated properly), but only in case we use async_std as runtime and not in case of Tokio as runtime!!!
        }
        Err(err) => {
            // We also shut down in case of error.
            eprintln!("Unable to listen for the shutdown signal: {}", err);
        }
    }

    // Send the shutdown signal to application and wait for it to shut down
    // Implement broadcasting or sending the shutdown signal...

    // Ok(())
}
