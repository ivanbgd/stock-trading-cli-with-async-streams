use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "Stock-Tracking CLI with Async Streams")]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// From
    #[arg(short, long)]
    pub from: String,

    /// Symbols
    #[arg(short, long, default_value = "AAPL,AMZN,GOOG,MSFT")]
    pub symbols: String,
}
