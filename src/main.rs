use stock::logic::main_loop;
use stock_trading_cli_with_async_streams as stock;
use stock_trading_cli_with_async_streams::types::{MsgResponseType, UniversalMsgErrorType};

#[tokio::main]
// async fn main() -> Result<MsgResponseType, actix::MailboxError> {
async fn main() -> Result<MsgResponseType, UniversalMsgErrorType> {
    println!();

    main_loop().await?;

    Ok(())
}
