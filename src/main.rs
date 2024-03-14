use stock::logic::main_loop;
use stock_trading_cli_with_async_streams as stock;
use stock_trading_cli_with_async_streams::types::{MsgErrorType, MsgResponseType};

#[tokio::main]
// async fn main() -> Result<(), actix::MailboxError> {
async fn main() -> Result<MsgResponseType, MsgErrorType> {
    println!();

    main_loop().await?;

    Ok(())
}
