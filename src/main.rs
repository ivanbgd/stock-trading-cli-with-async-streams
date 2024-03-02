use stock::logic::main_loop;
use stock_trading_cli_with_async_streams as stock;

#[actix::main]
async fn main() -> std::io::Result<()> {
    println!();

    main_loop().await?;

    Ok(())
}
