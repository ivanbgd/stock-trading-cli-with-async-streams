use std::io::{Error, ErrorKind};

use clap::Parser;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use yahoo_finance_api as yahoo;

use crate::cli::Args;
use crate::constants::WINDOW_SIZE;

/// A trait to provide a common interface for all signal calculations
trait AsyncStockSignal {
    /// A signal's data type
    type SignalType;

    /// Calculate a signal on the provided series
    ///
    /// # Returns
    /// Calculated signal of the provided type, or `None` on error/invalid data
    async fn calculate(&self, series: &[f64]) -> Option<Self::SignalType>;
}

/// Find the minimum in a series of `f64`
struct MinPrice {}

impl AsyncStockSignal for MinPrice {
    type SignalType = f64;

    /// Returns the minimum in a series of `f64` or `None` if it's empty.
    async fn calculate(&self, series: &[f64]) -> Option<Self::SignalType> {
        if series.is_empty() {
            None
        } else {
            Some(
                series
                    .iter()
                    .fold(f64::MAX, |min_elt, curr_elt| min_elt.min(*curr_elt)),
            )
        }
    }
}

/// Find the maximum in a series of `f64`
struct MaxPrice {}

impl AsyncStockSignal for MaxPrice {
    type SignalType = f64;

    /// Returns the maximum in a series of `f64` or `None` if it's empty.
    async fn calculate(&self, series: &[f64]) -> Option<Self::SignalType> {
        if series.is_empty() {
            None
        } else {
            Some(
                series
                    .iter()
                    .fold(f64::MIN, |max_elt, curr_elt| max_elt.max(*curr_elt)),
            )
        }
    }
}

/// Calculates the absolute and relative difference between the last and the first element of an f64 series.
///
/// The relative difference is calculated as `(last - first) / first`.
struct PriceDifference {}

impl AsyncStockSignal for PriceDifference {
    type SignalType = (f64, f64);

    /// Calculates the absolute and relative difference between the last and the first element of an f64 series.
    ///
    /// The relative difference is calculated as `(last - first) / first`.
    ///
    /// # Returns
    /// A tuple of `(absolute, relative)` differences, or `None` if the series is empty.
    async fn calculate(&self, series: &[f64]) -> Option<Self::SignalType> {
        if series.is_empty() {
            None
        } else {
            let first = series.first().expect("Expected first.");
            let last = series.last().unwrap_or(first);

            let abs_diff = last - first;

            let first = if *first == 0.0 { 1.0 } else { *first };
            let rel_diff = abs_diff / first;

            Some((abs_diff, rel_diff))
        }
    }
}

/// Window function to create a simple moving average
struct WindowedSMA {
    window_size: usize,
}

impl AsyncStockSignal for WindowedSMA {
    type SignalType = Vec<f64>;

    /// Window function to create a simple moving average
    ///
    /// # Returns
    /// A vector with the series' windowed averages;
    /// or `None` in case the series is empty or window size <= 1.
    async fn calculate(&self, series: &[f64]) -> Option<Self::SignalType> {
        if !series.is_empty() && self.window_size > 1 {
            Some(
                series
                    .windows(self.window_size)
                    .map(|window| window.iter().sum::<f64>() / window.len() as f64)
                    .collect(),
            )
        } else {
            None
        }
    }
}

///
/// Retrieve data from a data source and extract the closing prices
///
/// Errors during download are mapped onto `io::Errors` as `InvalidData`.
///
async fn fetch_closing_data(
    symbol: &str,
    beginning: OffsetDateTime,
    end: OffsetDateTime,
) -> std::io::Result<Vec<f64>> {
    let provider = yahoo::YahooConnector::new();

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

pub async fn main_logic() -> std::io::Result<()> {
    let args = Args::parse();
    let from = OffsetDateTime::parse(&args.from, &Rfc3339)
        .expect("The provided date or time format isn't correct.");
    let to = OffsetDateTime::now_utc();

    let min = MinPrice {};
    let max = MaxPrice {};
    let price_diff = PriceDifference {};
    let n_window_sma = WindowedSMA {
        window_size: WINDOW_SIZE,
    };

    // A simple way to output a CSV header
    println!("period start,symbol,price,change %,min,max,30d avg");

    for symbol in args.symbols.split(',') {
        let symbol = symbol.trim();
        let closes = fetch_closing_data(&symbol, from, to).await?;
        if !closes.is_empty() {
            // min/max of the period. unwrap() because those are Option types
            let period_min: f64 = min.calculate(&closes).await.unwrap();
            let period_max: f64 = max.calculate(&closes).await.unwrap();
            let last_price = *closes.last().unwrap_or(&0.0);
            let (_, pct_change) = price_diff.calculate(&closes).await.unwrap_or((0.0, 0.0));
            let sma = n_window_sma.calculate(&closes).await.unwrap_or_default();

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

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[async_std::test]
    async fn test_min_price_calculate() {
        let signal = MinPrice {};
        assert_eq!(signal.calculate(&[]).await, None);
        assert_eq!(signal.calculate(&[1.0]).await, Some(1.0));
        assert_eq!(signal.calculate(&[1.0, 0.0]).await, Some(0.0));
        assert_eq!(
            signal
                .calculate(&[2.0, 3.0, 5.0, 6.0, 1.0, 2.0, 10.0])
                .await,
            Some(1.0)
        );
        assert_eq!(
            signal.calculate(&[0.0, 3.0, 5.0, 6.0, 1.0, 2.0, 1.0]).await,
            Some(0.0)
        );
    }

    #[async_std::test]
    async fn test_max_price_calculate() {
        let signal = MaxPrice {};
        assert_eq!(signal.calculate(&[]).await, None);
        assert_eq!(signal.calculate(&[1.0]).await, Some(1.0));
        assert_eq!(signal.calculate(&[1.0, 0.0]).await, Some(1.0));
        assert_eq!(
            signal
                .calculate(&[2.0, 3.0, 5.0, 6.0, 1.0, 2.0, 10.0])
                .await,
            Some(10.0)
        );
        assert_eq!(
            signal.calculate(&[0.0, 3.0, 5.0, 6.0, 1.0, 2.0, 1.0]).await,
            Some(6.0)
        );
    }

    #[async_std::test]
    async fn test_price_difference_calculate() {
        let signal = PriceDifference {};
        assert_eq!(signal.calculate(&[]).await, None);
        assert_eq!(signal.calculate(&[1.0]).await, Some((0.0, 0.0)));
        assert_eq!(signal.calculate(&[1.0, 0.0]).await, Some((-1.0, -1.0)));
        assert_eq!(
            signal
                .calculate(&[2.0, 3.0, 5.0, 6.0, 1.0, 2.0, 10.0])
                .await,
            Some((8.0, 4.0))
        );
        assert_eq!(
            signal.calculate(&[0.0, 3.0, 5.0, 6.0, 1.0, 2.0, 1.0]).await,
            Some((1.0, 1.0))
        );
    }

    #[async_std::test]
    async fn test_windowed_sma_calculate() {
        let series = vec![2.0, 4.5, 5.3, 6.5, 4.7];

        let signal = WindowedSMA { window_size: 3 };
        assert_eq!(
            signal.calculate(&series).await,
            Some(vec![3.9333333333333336, 5.433333333333334, 5.5])
        );

        let signal = WindowedSMA { window_size: 5 };
        assert_eq!(signal.calculate(&series).await, Some(vec![4.6]));

        let signal = WindowedSMA { window_size: 10 };
        assert_eq!(signal.calculate(&series).await, Some(vec![]));
    }
}
