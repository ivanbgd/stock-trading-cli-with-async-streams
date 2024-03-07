use actix::{Actor, Context, ContextFutureSpawner, Handler, Message, WrapFuture};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use yahoo_finance_api as yahoo;

use crate::constants::WINDOW_SIZE;
use crate::signals::{AsyncStockSignal, MaxPrice, MinPrice, PriceDifference, WindowedSMA};

/// The [`QuoteRequestMsg`] message
///
/// It contains a `symbol`, and `from` and `to` fields.
///
/// Expected response is a vector of closing prices for the symbol for the period. todo: perhaps return nothing but call/spawn a ProcessorWriterActor?!
#[derive(Message)]
// #[rtype(result = "Result<(), std::io::Error>")] todo remove
#[rtype(result = "()")]
pub struct QuoteRequestMsg {
    pub symbol: String,
    pub from: OffsetDateTime,
    pub to: OffsetDateTime,
}

/// Actor that downloads stock data for a specified symbol and period
///
/// It is stateless - it doesn't contain any user data.
pub struct FetchActor;

impl Actor for FetchActor {
    type Context = Context<Self>;
}

/// The [`QuoteRequestMsg`] message handler for the [`FetchActor`] actor
impl Handler<QuoteRequestMsg> for FetchActor {
    // type Result = Result<Vec<f64>, ()>;
    // type Result = ResponseFuture<Result<Vec<f64>, ()>>;
    // type Result = Result<(), std::io::Error>;
    type Result = ();

    /// The [`QuoteRequestMsg`] message handler for the [`FetchActor`] actor
    ///
    /// Spawns a new actor and sends it a [`SymbolClosesMsg`] message. TODO check!
    ///
    /// The message contains a `symbol` and a vector of closing prices in case there was no error
    /// when fetching the data, or an empty vector in case of an error, in which case it prints
    /// the error message to `stderr`.
    ///
    /// So, in case of an API error for a symbol, when trying to fetch its data,
    /// we don't break the program but rather continue.
    fn handle(&mut self, msg: QuoteRequestMsg, ctx: &mut Self::Context) -> Self::Result {
        let symbol = msg.symbol;
        let from = msg.from;
        let to = msg.to;

        let provider = yahoo::YahooConnector::new();

        async move {
            let msg = match fetch_closing_data(&symbol, from, to, &provider).await {
                Ok(closes) => SymbolClosesMsg {
                    symbol,
                    closes,
                    from,
                },
                Err(err) => {
                    println!(
                        "There was an API error \"{}\" while fetching data for the symbol \"{}\"; skipping the symbol.",
                        err, symbol
                    );
                    SymbolClosesMsg {
                        symbol,
                        closes: vec![],
                        from,
                    }
                }
            };

            // Spawn another Actor and send it the message.
            let proc_writer_address = ProcessorWriterActor.start();
            let _ = proc_writer_address
                .send(msg)
                .await
                .expect("Couldn't send a message.");
        }
        .into_actor(self)
        .spawn(ctx);
    }
}

/// The [`SymbolClosesMsg`] message
///
/// It contains a `symbol`, and a `Vec<f64>` with closing prices for that symbol.
/// It contains a `symbol`, and a `Vec<yahoo::Quote>` for that symbol. todo
///
/// There is no expected response.
#[derive(Message)]
#[rtype(result = "()")]
pub struct SymbolClosesMsg {
    pub symbol: String,
    pub closes: Vec<f64>,
    pub from: OffsetDateTime,
    // pub quotes: Vec<yahoo::Quote>, // todo
}

pub struct ProcessorWriterActor;

impl Actor for ProcessorWriterActor {
    type Context = Context<Self>;
}

impl Handler<SymbolClosesMsg> for ProcessorWriterActor {
    type Result = ();

    fn handle(&mut self, msg: SymbolClosesMsg, ctx: &mut Self::Context) -> Self::Result {
        let symbol = msg.symbol;
        let closes = msg.closes;
        let from = msg.from;

        async move {
            if !closes.is_empty() {
                let min = MinPrice {};
                let max = MaxPrice {};
                let price_diff = PriceDifference {};
                let n_window_sma = WindowedSMA {
                    window_size: WINDOW_SIZE,
                };

                let period_min: f64 = min.calculate(&closes).await.unwrap_or_default();
                let period_max: f64 = max.calculate(&closes).await.unwrap_or_default();
                let last_price = *closes.last().expect("Expected non-empty closes.");
                let (_, pct_change) = price_diff.calculate(&closes).await.unwrap_or((0., 0.));
                let sma = n_window_sma.calculate(&closes).await.unwrap_or(vec![]);

                // A simple way to output CSV data
                println!(
                    "{},{},${:.2},{:.2}%,${:.2},${:.2},${:.2}",
                    OffsetDateTime::format(from, &Rfc3339).expect("Couldn't format 'from'."),
                    symbol,
                    last_price,
                    pct_change * 100.0,
                    period_min,
                    period_max,
                    sma.last().unwrap_or(&0.0)
                );
            }
        }
        .into_actor(self)
        .spawn(ctx);
    }
}

// todo use this
// /// The [`QuoteRequestMsg`] message
// #[derive(Message)]
// #[rtype(result = "Vec<Vec<f64>>")]
// pub struct QuoteRequestMsg {
//     pub chunk: Vec<String>,
//     pub from: OffsetDateTime,
//     pub to: OffsetDateTime,
// }
//
// /// The [`QuoteRequestMsg`] message handler for the [`FetchActor`] actor
// impl Handler<QuoteRequestMsg> for FetchActor {
//     type Result = Vec<Vec<f64>>;
//
//     fn handle(&mut self, msg: QuoteRequestMsg, ctx: &mut Self::Context) -> Self::Result {
//         let symbols = msg.chunk;
//         let from = msg.from;
//         let to = msg.to;
//
//         let provider = yahoo::YahooConnector::new();
//
//         let mut result: Vec<Vec<f64>> = vec![];
//
//         async move {
//             for symbol in symbols {
//                 let closes = fetch_closing_data(&symbol, from, to, &provider)
//                     .await
//                     .unwrap_or_default();
//                 result.push(closes);
//             }
//         }
//         .into_actor(self)
//         .spawn(ctx);
//
//         result
//     }
// }

/// Retrieve data for a single `symbol` from a data source (`provider`) and extract the closing prices
///
/// # Returns
/// - Vector of closing prices in case of no error, or,
/// - [`yahoo::YahooError`](https://docs.rs/yahoo_finance_api/2.1.0/yahoo_finance_api/enum.YahooError.html)
///   in case of an error.
async fn fetch_closing_data(
    symbol: &str,
    from: OffsetDateTime,
    to: OffsetDateTime,
    provider: &yahoo::YahooConnector,
) -> Result<Vec<f64>, yahoo::YahooError> {
    let yresponse = provider.get_quote_history(symbol, from, to).await?;
    let mut quotes = yresponse.quotes()?;

    let mut result = vec![];
    if !quotes.is_empty() {
        quotes.sort_by_cached_key(|k| k.timestamp);
        result = quotes.iter().map(|q| q.adjclose).collect();
    }

    Ok(result)
}

// todo consider this for chunks
//
//
//
//
// /// Convenience function that chains together the entire processing chain
// ///
// /// We don't need to return anything.
// pub async fn _handle_symbol_data(
//     // symbols: &[&str],
//     symbols: &[String],
//     from: OffsetDateTime,
//     to: OffsetDateTime,
// ) {
//     let provider = yahoo::YahooConnector::new();
//
//     for symbol in symbols {
//         let closes = fetch_closing_data(symbol, from, to, &provider)
//             .await
//             .unwrap_or_default();
//
//         if !closes.is_empty() {
//             let min = MinPrice {};
//             let max = MaxPrice {};
//             let price_diff = PriceDifference {};
//             let n_window_sma = WindowedSMA {
//                 window_size: WINDOW_SIZE,
//             };
//
//             let period_min: f64 = min.calculate(&closes).await.unwrap_or_default();
//             let period_max: f64 = max.calculate(&closes).await.unwrap_or_default();
//             let last_price = *closes.last().expect("Expected non-empty closes.");
//             let (_, pct_change) = price_diff.calculate(&closes).await.unwrap_or((0., 0.));
//             let sma = n_window_sma.calculate(&closes).await.unwrap_or(vec![]);
//
//             // A simple way to output CSV data
//             println!(
//                 "{},{},${:.2},{:.2}%,${:.2},${:.2},${:.2}",
//                 OffsetDateTime::format(from, &Rfc3339).expect("Couldn't format 'from'."),
//                 symbol,
//                 last_price,
//                 pct_change * 100.0,
//                 period_min,
//                 period_max,
//                 sma.last().unwrap_or(&0.0)
//             );
//         }
//     }
// }
