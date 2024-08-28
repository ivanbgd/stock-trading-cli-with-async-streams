use std::fmt::Debug;

use clap::{Parser, ValueEnum};

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

    /// Implementation variant
    #[arg(long, default_value = "my-actors-no-rayon")]
    pub variant: ImplementationVariant,
}

#[derive(Clone, Debug, ValueEnum)]
#[non_exhaustive]
pub enum ImplementationVariant {
    MyActorsNoRayon,
    MyActorsRayon,
    ActixActorsNoRayon,
    ActixActorsRayon,
    NoActorsNoRayon,
    NoActorsRayon,
}

// This is what `#[derive(Debug)]` does:

// impl Debug for ImplementationVariant {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         match self {
//             ImplementationVariant::MyActorsNoRayon => write!(f, "MyActorsNoRayon"),
//             ImplementationVariant::MyActorsRayon => write!(f, "MyActorsRayon"),
//             ImplementationVariant::ActixActorsNoRayon => write!(f, "ActixActorsNoRayon"),
//             ImplementationVariant::ActixActorsRayon => write!(f, "ActixActorsRayon"),
//             ImplementationVariant::NoActorsNoRayon => write!(f, "NoActorsNoRayon"),
//             ImplementationVariant::NoActorsRayon => write!(f, "NoActorsRayon"),
//         }
//     }
// }

// We don't need to implement std::fmt::Display thanks to ValueEnum.

// impl Display for ImplementationVariant {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         write!(f, "{:?}", self)
//     }
// }
