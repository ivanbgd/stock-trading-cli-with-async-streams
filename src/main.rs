use stock::logic::main_loop;
use stock_trading_cli_with_async_streams as stock;

// #[actix::main]
// async fn main() -> Result<(), actix::MailboxError> {
fn main() -> Result<(), actix::MailboxError> {
    println!();

    // main_loop().await?;
    main_loop().expect("MAIN!");

    Ok(())
}
