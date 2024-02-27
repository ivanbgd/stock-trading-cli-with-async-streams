use std::time::Instant;

use stock_trading_cli_with_async_streams::logic::main_logic;

#[async_std::main]
async fn main() -> std::io::Result<()> {
    let start = Instant::now();

    println!();

    main_logic().await?;

    println!("\nTook {:.3?} to complete.", start.elapsed());

    Ok(())
}
