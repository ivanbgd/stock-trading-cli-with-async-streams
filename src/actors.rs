use std::io::{Error, ErrorKind};

use actix::{Actor, Context, ContextFutureSpawner, Handler, Message, ResponseFuture, WrapFuture};
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
// #[rtype(result = "Result<Vec<f64>, ()>")]
#[rtype(result = "Vec<f64>")]
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
    type Result = ResponseFuture<Vec<f64>>;

    fn handle(&mut self, msg: QuoteRequestMsg, _ctx: &mut Self::Context) -> Self::Result {
        let symbol = msg.symbol;
        let from = msg.from;
        let to = msg.to;

        // let provider = yahoo::YahooConnector::new();

        Box::pin(async move {
            let closes = fetch_closing_data(&symbol, from, to) //, &provider)
                .await
                .unwrap_or_default();
            closes
            // Ok(closes)
        })
    }
}

/// The [`SymbolClosesMsg`] message
///
/// It contains a `symbol`, and a `Vec<yahoo::Quote>`. todo
///
/// There is no expected response.
#[derive(Message)]
#[rtype(result = "()")]
pub struct SymbolClosesMsg {
    pub closes: Vec<f64>,
    // pub quotes: Vec<yahoo::Quote>,
    pub symbol: String,
    pub from: OffsetDateTime,
}

pub struct ProcessorWriterActor;

impl Actor for ProcessorWriterActor {
    type Context = Context<Self>;
}

impl Handler<SymbolClosesMsg> for ProcessorWriterActor {
    type Result = ();

    fn handle(&mut self, msg: SymbolClosesMsg, ctx: &mut Self::Context) -> Self::Result {
        let symbol = msg.symbol;
        // let quotes = msg.quotes;

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

/// Retrieve data from a data source and extract the closing prices
///
/// Errors during download are mapped onto `io::Errors` as `InvalidData`.
async fn fetch_closing_data(
    symbol: &str,
    beginning: OffsetDateTime,
    end: OffsetDateTime,
    // provider: &yahoo::YahooConnector,
) -> std::io::Result<Vec<f64>> {
    let provider = yahoo::YahooConnector::new(); //

    let response = provider
        .get_quote_history(symbol, beginning, end)
        .await
        .map_err(|_| Error::from(ErrorKind::InvalidData))?;
    // dbg!(&response);
    let mut quotes = response
        .quotes()
        .map_err(|_| Error::from(ErrorKind::InvalidData))?;
    let mut result = vec![];
    if !quotes.is_empty() {
        quotes.sort_by_cached_key(|k| k.timestamp);
        result = quotes.iter().map(|q| q.adjclose).collect();
    }
    // dbg!(&result);
    Ok(result)
}

/// Convenience function that chains together the entire processing chain
///
/// We don't need to return anything.
pub async fn _handle_symbol_data(
    // symbols: &[&str],
    symbols: &[String],
    beginning: OffsetDateTime,
    end: OffsetDateTime,
) {
    let _provider = yahoo::YahooConnector::new();

    for symbol in symbols {
        let closes = fetch_closing_data(symbol, beginning, end) //, &provider)
            .await
            .unwrap_or_default();

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
                OffsetDateTime::format(beginning, &Rfc3339).expect("Couldn't format 'from'."),
                symbol,
                last_price,
                pct_change * 100.0,
                period_min,
                period_max,
                sma.last().unwrap_or(&0.0)
            );
        }
    }
}
