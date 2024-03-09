use std::fs::File;
use std::io::{BufWriter, Write};

use actix::{
    Actor, ActorContext, Addr, Context, ContextFutureSpawner, Handler, Message, WrapFuture,
};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use yahoo_finance_api as yahoo;

// use crate::constants::{CSV_HEADER, WINDOW_SIZE}; todo
use crate::constants::{CSV_HEADER, WINDOW_SIZE};
use crate::signals::{AsyncStockSignal, MaxPrice, MinPrice, PriceDifference, WindowedSMA};

/// The [`QuoteRequestMsg`] message
///
/// It contains a `symbol`, and `from` and `to` fields.
///
/// It also contains a [`WriterActor`] address.
///
/// There is no expected response.
#[derive(Message)]
#[rtype(result = "()")]
pub struct QuoteRequestMsg {
    pub symbol: String,
    pub from: OffsetDateTime,
    pub to: OffsetDateTime,
    pub writer_address: Addr<WriterActor>,
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
    type Result = ();

    /// The [`QuoteRequestMsg`] message handler for the [`FetchActor`] actor
    ///
    /// Spawns a new [`ProcessorActor`] and sends it a [`SymbolClosesMsg`] message.
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
        let writer_address = msg.writer_address;

        let provider = yahoo::YahooConnector::new();

        async move {
            let closes_msg = match fetch_closing_data(&symbol, from, to, &provider).await {
                Ok(closes) => SymbolClosesMsg {
                    symbol,
                    closes,
                    from,
                    writer_address,
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
                        writer_address,
                    }
                }
            };

            // Spawn another Actor and send it the message.
            let proc_address = ProcessorActor.start();
            let _ = proc_address
                .send(closes_msg)
                .await
                .expect("Couldn't send a message to the ProcessorActor.");
        }
        .into_actor(self)
        .spawn(ctx);
    }
}

/// The [`SymbolClosesMsg`] message
///
/// It contains a `symbol`, a `Vec<f64>` with closing prices for that symbol,
/// and the starting date and time `from` field.
///
/// It also contains a [`WriterActor`] address.
///
/// There is no expected response.
#[derive(Message)]
#[rtype(result = "()")]
pub struct SymbolClosesMsg {
    pub symbol: String,
    pub closes: Vec<f64>,
    pub from: OffsetDateTime,
    pub writer_address: Addr<WriterActor>,
}

/// Actor for creating performance indicators from fetched stock data
struct ProcessorActor;

impl Actor for ProcessorActor {
    type Context = Context<Self>;
}

/// The [`SymbolClosesMsg`] message handler for the [`ProcessorActor`] actor
impl Handler<SymbolClosesMsg> for ProcessorActor {
    type Result = ();

    /// The [`SymbolClosesMsg`] message handler for the [`ProcessorActor`] actor
    ///
    /// Sends a [`PerformanceIndicatorsMsg`] message to the [`WriterActor`],
    /// whose address it gets from the [`SymbolClosesMsg`] message.
    fn handle(&mut self, msg: SymbolClosesMsg, ctx: &mut Self::Context) -> Self::Result {
        let symbol = msg.symbol;
        let closes = msg.closes;
        let from = msg.from;
        let writer_address = msg.writer_address;

        async move {
            if !closes.is_empty() {
                let min = MinPrice {};
                let max = MaxPrice {};
                let price_diff = PriceDifference {};
                let n_window_sma = WindowedSMA {
                    window_size: WINDOW_SIZE,
                };

                let from = OffsetDateTime::format(from, &Rfc3339).expect("Couldn't format 'from'.");
                let last_price = *closes.last().expect("Expected non-empty closes.");
                let (_, pct_change) = price_diff.calculate(&closes).await.unwrap_or((0., 0.));
                let pct_change = pct_change * 100.0;
                let period_min: f64 = min.calculate(&closes).await.unwrap_or_default();
                let period_max: f64 = max.calculate(&closes).await.unwrap_or_default();
                let sma = n_window_sma.calculate(&closes).await.unwrap_or(vec![]);
                let sma = *sma.last().unwrap_or(&0.0);

                let perf_ind_msg = PerformanceIndicatorsMsg {
                    from: from.clone(),
                    symbol: symbol.clone(),
                    last_price,
                    pct_change,
                    period_min,
                    period_max,
                    sma,
                };

                // // Spawn another Actor and send it the message.
                // let writer_address = WriterActor {
                //     file_name: "output.csv".to_string(),
                //     writer: None,
                // }
                // .start(); todo remove

                // Send the message to the single writer actor.
                let _ = writer_address
                    .send(perf_ind_msg)
                    .await
                    .expect("Couldn't send a message to the WriterActor.");

                // A simple way to output CSV data
                println!(
                    "{},{},${:.2},{:.2}%,${:.2},${:.2},${:.2}",
                    from, symbol, last_price, pct_change, period_min, period_max, sma,
                );
            } else {
                eprintln!("Got no data for the symbol \"{}\".", symbol);
            }
        }
        .into_actor(self)
        .spawn(ctx);
    }
}

/// The [`PerformanceIndicatorsMsg`] message
///
/// It contains a `symbol` and calculated performance indicators for that symbol.
///
/// There is no expected response.
#[derive(Message)]
#[rtype(result = "()")]
pub struct PerformanceIndicatorsMsg {
    pub from: String,
    pub symbol: String,
    pub last_price: f64,
    pub pct_change: f64,
    pub period_min: f64,
    pub period_max: f64,
    pub sma: f64,
}

/// Actor for writing calculated performance indicators for fetched stock data into a CSV file
pub struct WriterActor {
    pub file_name: String,
    pub writer: Option<BufWriter<File>>,
}

impl Actor for WriterActor {
    type Context = Context<Self>;
    // type Context = SyncContext<Self>; todo remove

    fn started(&mut self, _ctx: &mut Self::Context) {
        let mut file = File::create(&self.file_name)
            .unwrap_or_else(|_| panic!("Could not open target file \"{}\".", self.file_name));
        let _ = writeln!(&mut file, "{}", CSV_HEADER);
        self.writer = Some(BufWriter::new(file));
        println!("WriterActor is started.");
    }

    fn stopped(&mut self, ctx: &mut Self::Context) {
        if let Some(writer) = &mut self.writer {
            writer
                .flush()
                .expect("Failed to flush writer. Data loss :(")
        };
        ctx.stop();
        println!("WriterActor is flushed and properly stopped.");
    }
}

/// The [`PerformanceIndicatorsMsg`] message handler for the [`WriterActor`] actor
impl Handler<PerformanceIndicatorsMsg> for WriterActor {
    type Result = ();

    fn handle(&mut self, msg: PerformanceIndicatorsMsg, ctx: &mut Self::Context) -> Self::Result {
        // let mut writer = &self.writer;

        // async move {
        if let Some(file) = &mut self.writer {
            let _ = writeln!(
                file,
                "{},{},${:.2},{:.2}%,${:.2},${:.2},${:.2}",
                msg.from,
                msg.symbol,
                msg.last_price,
                msg.pct_change,
                msg.period_min,
                msg.period_max,
                msg.sma,
            );
        }
        // }
        // .into_actor(self)
        // .spawn(ctx);
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
// pub async fn handle_symbol_data(
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
//             let pct_change = pct_change * 100.0;
//             let sma = n_window_sma.calculate(&closes).await.unwrap_or(vec![]);
//
//             // A simple way to output CSV data
//             println!(
//                 "{},{},${:.2},{:.2}%,${:.2},${:.2},${:.2}",
//                 OffsetDateTime::format(from, &Rfc3339).expect("Couldn't format 'from'."),
//                 symbol,
//                 last_price,
//                 pct_change,
//                 period_min,
//                 period_max,
//                 sma.last().unwrap_or(&0.0)
//             );
//         }
//     }
// }
