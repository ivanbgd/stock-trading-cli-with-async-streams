use std::io::{Error, ErrorKind};

// use actix::{Actor, Context, ContextFutureSpawner, Handler, Message, WrapFuture};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use yahoo_finance_api as yahoo;

use crate::constants::WINDOW_SIZE;
use crate::signals::{AsyncStockSignal, MaxPrice, MinPrice, PriceDifference, WindowedSMA};

// /// A single actor that downloads data, processes them and prints the results to console
// pub struct MultiActor;
//
// impl Actor for MultiActor {
//     type Context = Context<Self>;
// }
//
// #[derive(Message)]
// #[rtype(result = "()")]
// pub struct QuoteRequest {
//     pub chunk: Vec<String>,
//     pub from: OffsetDateTime,
//     pub to: OffsetDateTime,
// }
//
// impl Handler<QuoteRequest> for MultiActor {
//     type Result = ();
//
//     fn handle(&mut self, msg: QuoteRequest, ctx: &mut Self::Context) -> Self::Result {
//         let symbols = msg.chunk;
//         let from = msg.from;
//         let to = msg.to;
//
//         async move {
//             for symbol in symbols {
//                 handle_symbol_data(&symbol, from, to).await;
//             }
//         }
//         .into_actor(self)
//         .spawn(ctx);
//     }
// }

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

#[cfg(test)]
mod tests {
    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;

    use super::fetch_closing_data;

    /// Devised so we can test whether working with a single `provider: yahoo::YahooConnector` is correct.
    ///
    /// We first worked with a new provider for every single symbol handling.
    ///
    /// This test function is not mocked, so it requires Internet connection and Yahoo! Finance API to be available.
    #[async_std::test]
    async fn aapl_closing_data() {
        let symbol = "AAPL";
        let from = OffsetDateTime::parse("2024-01-01T12:00:00+00:00", &Rfc3339).unwrap();
        let to = OffsetDateTime::parse("2024-02-29T15:51:29+00:00", &Rfc3339).unwrap();
        let provider = yahoo_finance_api::YahooConnector::new();
        let closes = fetch_closing_data(symbol, from, to, &provider)
            .await
            .unwrap();
        assert_eq!((closes.first().unwrap() * 100.0).round() / 100.0, 185.640);
    }
}
