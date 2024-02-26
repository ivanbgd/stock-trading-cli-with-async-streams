use stock_trading_cli_with_async_streams::logic::main_logic;

fn main() -> std::io::Result<()> {
    main_logic()?;

    Ok(())
}
