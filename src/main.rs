use stock::logic::main_loop;
use stock_trading_cli_with_async_streams as stock;

/// [actix-rt](https://crates.io/crates/actix-rt) is a
/// "Tokio-based single-threaded async runtime for the Actix ecosystem".
#[actix::main]
async fn main() -> std::io::Result<()> {
    println!();

    main_loop().await?;

    Ok(())
}
