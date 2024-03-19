use std::fs::File;
use std::io::{BufWriter, Error, ErrorKind, Write};

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use yahoo_finance_api as yahoo;

use crate::async_signals::{AsyncStockSignal, MaxPrice, MinPrice, PriceDifference, WindowedSMA};
use crate::constants::{CSV_FILE_NAME, CSV_HEADER, WINDOW_SIZE};

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
    symbols: &[String],
    beginning: OffsetDateTime,
    end: OffsetDateTime,
) -> Vec<String> {
    let provider = yahoo::YahooConnector::new();

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

            let period_min: f64 = min.calculate(&closes).await.unwrap_or_default();
            let period_max: f64 = max.calculate(&closes).await.unwrap_or_default();
            let last_price = *closes.last().expect("Expected non-empty closes.");
            let (_, pct_change) = price_diff.calculate(&closes).await.unwrap_or((0., 0.));
            let sma = n_window_sma.calculate(&closes).await.unwrap_or(vec![]);

            let row = format!(
                "{},{},${:.2},{:.2}%,${:.2},${:.2},${:.2}",
                OffsetDateTime::format(beginning, &Rfc3339).expect("Couldn't format 'from'."),
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

    rows
}

pub fn start_writer() -> Option<BufWriter<File>> {
    let file_name = CSV_FILE_NAME.to_string();
    let mut file = File::create(&file_name)
        .unwrap_or_else(|_| panic!("Could not open target file \"{}\".", file_name));
    let _ = writeln!(&mut file, "{}", CSV_HEADER);
    let writer = Some(BufWriter::new(file));
    println!("Writer is started.");

    writer
}

pub fn write_to_csv(mut writer: &mut Option<BufWriter<File>>, all_rows: Vec<Vec<String>>) {
    if let Some(file) = &mut writer {
        // let rows = rows.iter().flatten();
        for rows in all_rows {
            for row in rows {
                let _ = writeln!(file, "{}", row);
            }
        }
        file.flush().expect("Failed to flush to file. Data loss :/");
    }
}

pub fn stop_writer(mut writer: Option<BufWriter<File>>) {
    if let Some(writer) = &mut writer {
        writer
            .flush()
            .expect("Failed to flush writer. Data loss :(")
    };
    println!("Writer is flushed and properly stopped.");
}
