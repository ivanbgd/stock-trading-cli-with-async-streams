use std::io::{Error, ErrorKind};

use actix::{Actor, Context, ContextFutureSpawner, Handler, Message, WrapFuture};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use yahoo_finance_api as yahoo;

use crate::constants::WINDOW_SIZE;
use crate::signals::{AsyncStockSignal, MaxPrice, MinPrice, PriceDifference, WindowedSMA};

/// A single actor that downloads data, processes them and prints the results to console
///
/// It is stateless - it doesn't contain any user data.
pub struct MultiActor;

/// Implementation of the `Actor` trait for the [`MultiActor`] actor
impl Actor for MultiActor {
    type Context = Context<Self>;
    // type Context = SyncContext<Self>; // Doesn't work.
}

/// The [`QuoteRequest`] message
#[derive(Message)]
#[rtype(result = "()")]
pub struct QuoteRequest {
    pub chunk: Vec<String>,
    pub from: OffsetDateTime,
    pub to: OffsetDateTime,
}

/// The [`QuoteRequest`] message handler for the [`MultiActor`] actor
impl Handler<QuoteRequest> for MultiActor {
    type Result = ();

    fn handle(&mut self, msg: QuoteRequest, ctx: &mut Self::Context) -> Self::Result {
        let symbols = msg.chunk;
        let from = msg.from;
        let to = msg.to;

        async move {
            // for symbol in symbols {
            handle_symbol_data(&symbols, from, to).await;
            // }
        }
        .into_actor(self)
        .spawn(ctx);
    }
}

/// Retrieve data from a data source and extract the closing prices
///
/// Errors during download are mapped onto `io::Errors` as `InvalidData`.
async fn fetch_closing_data(
    symbol: &str,
    beginning: OffsetDateTime,
    end: OffsetDateTime,
    provider: &yahoo::YahooConnector,
) -> std::io::Result<Vec<f64>> {
    let response = provider
        .get_quote_history(symbol, beginning, end)
        .await
        .map_err(|_| Error::from(ErrorKind::InvalidData))?;
    let mut quotes = response
        .quotes()
        .map_err(|_| Error::from(ErrorKind::InvalidData))?;
    if !quotes.is_empty() {
        quotes.sort_by_cached_key(|k| k.timestamp);
        Ok(quotes.iter().map(|q| q.adjclose).collect())
    } else {
        Ok(vec![])
    }
}

/// Convenience function that chains together the entire processing chain
///
/// We don't need to return anything.
pub async fn handle_symbol_data(
    // symbols: &[&str],
    symbols: &[String],
    beginning: OffsetDateTime,
    end: OffsetDateTime,
) {
    let provider = yahoo::YahooConnector::new();

    for symbol in symbols {
        let closes = fetch_closing_data(symbol, beginning, end, &provider)
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
