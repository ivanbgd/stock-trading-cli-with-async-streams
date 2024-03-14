use stock::logic::main_loop;
use stock_trading_cli_with_async_streams as stock;

#[tokio::main]
// async fn main() -> Result<MsgResponseType, actix::MailboxError> {
async fn main() {
    println!();

    let _ = main_loop().await;
    // main_loop().await?;

    // Ok(())
}
