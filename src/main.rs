use stock::logic::main_loop;
use stock_trading_cli_with_async_streams as stock;

#[tokio::main]
async fn main() -> Result<(), actix::MailboxError> {
    println!();

    main_loop().await?;

    Ok(())
}
