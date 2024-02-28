use stock_trading_cli_with_async_streams::logic::main_loop;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    println!();

    main_loop().await?;

    Ok(())
}
