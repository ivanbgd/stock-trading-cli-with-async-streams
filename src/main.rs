use stock_trading_cli_with_async_streams::logic::main_logic;

#[async_std::main]
async fn main() -> std::io::Result<()> {
    main_logic().await?;

    Ok(())
}
