use std::fs::File;
use std::io::{BufWriter, Write};

use anyhow::{Context, Result};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use yahoo_finance_api as yahoo;

use crate::async_signals::{AsyncStockSignal, MaxPrice, MinPrice, PriceDifference, WindowedSMA};
use crate::constants::{CSV_FILE_NAME, CSV_HEADER, WINDOW_SIZE};

/// Retrieves data for a single symbol from a data provider and extracts the closing prices
///
/// # Returns
/// Vector of closing prices for the symbol and for the given period, if available
///
/// # Errors
/// - [yahoo_finance_api::YahooError](https://docs.rs/yahoo_finance_api/2.2.1/yahoo_finance_api/enum.YahooError.html)
async fn fetch_closing_data(
    symbol: &str,
    beginning: OffsetDateTime,
    end: OffsetDateTime,
    provider: &yahoo::YahooConnector,
) -> Result<Vec<f64>> {
    let response = provider.get_quote_history(symbol, beginning, end).await?;
    let mut quotes = response.quotes()?;
    if !quotes.is_empty() {
        quotes.sort_by_cached_key(|k| k.timestamp);
        Ok(quotes.iter().map(|q| q.adjclose).collect())
    } else {
        Ok(vec![])
    }
}

/// Convenience function that chains together the entire processing chain
///
/// # Returns
/// Vector of rows with results
///
/// # Errors
/// - [yahoo_finance_api::YahooError](https://docs.rs/yahoo_finance_api/2.2.1/yahoo_finance_api/enum.YahooError.html)
pub async fn handle_symbol_data(
    symbols: &[String],
    beginning: OffsetDateTime,
    end: OffsetDateTime,
) -> Result<Vec<String>> {
    let from = OffsetDateTime::format(beginning, &Rfc3339).context("Couldn't format 'from'.")?;

    // Provide some context, which is a list of symbols that were not fetched and which will be skipped.
    let provider = yahoo::YahooConnector::new().context(format!(
        "Couldn't connect to the Yahoo! API for the following chunk of symbols: {:?}",
        symbols
    ))?;
    // Alternatively, print them a little prettier.
    // let provider = match yahoo::YahooConnector::new() {
    //     Ok(connector) => connector,
    //     Err(_) => {
    //         let mut rows = vec![];
    //         for symbol in symbols {
    //             rows.push(format!("{},{}", from, symbol));
    //         }
    //         return Ok(rows);
    //     }
    // };

    let mut rows = vec![];

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

            let last_price = *closes.last().expect("Expected non-empty closes.");
            let (_, pct_change) = price_diff.calculate(&closes).await.unwrap_or((0., 0.));
            let period_min: f64 = min.calculate(&closes).await.unwrap_or_default();
            let period_max: f64 = max.calculate(&closes).await.unwrap_or_default();
            let sma = n_window_sma.calculate(&closes).await.unwrap_or(vec![]);

            let row = format!(
                "{},{},${:.2},{:.2}%,${:.2},${:.2},${:.2}",
                from,
                symbol,
                last_price,
                pct_change * 100.0,
                period_min,
                period_max,
                sma.last().unwrap_or(&0.0)
            );

            // A simple way to print CSV data
            println!("{}", row);

            rows.push(row);
        }
    }

    Ok(rows)
}

pub fn start_writer() -> Result<Option<BufWriter<File>>> {
    let file_name = CSV_FILE_NAME.to_string();
    let mut file = File::create(&file_name)
        .context(format!("Could not open target file \"{}\".", file_name))?;
    let _ = writeln!(&mut file, "{}", CSV_HEADER);
    let writer = Some(BufWriter::new(file));
    println!("Writer is started.");

    Ok(writer)
}

pub fn write_to_csv(
    mut writer: &mut Option<BufWriter<File>>,
    all_rows: Vec<&Result<Vec<String>>>,
) -> Result<()> {
    if let Some(file) = &mut writer {
        // let rows = all_rows.into_iter().flatten();
        for rows in all_rows {
            let rows = rows.as_ref().expect("Expected some rows to write.");
            for row in rows {
                let _ = writeln!(file, "{}", row);
            }
        }
        file.flush()
            .context("Failed to flush to file. Data loss :/")?;
    }
    Ok(())
}

pub fn stop_writer(mut writer: Option<BufWriter<File>>) -> Result<()> {
    if let Some(writer) = &mut writer {
        writer
            .flush()
            .context("Failed to flush writer. Data loss :(")?;
    };
    println!("Writer is flushed and properly stopped.");
    Ok(())
}
