use stock::logic::main_loop;
use stock_trading_cli_with_async_streams as stock;

#[xactor::main]
async fn main() -> xactor::Result<()> {
    println!();

    main_loop().await?;

    Ok(())
}
