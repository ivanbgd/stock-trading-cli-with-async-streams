use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};

use actix::{
    Actor, ActorContext, Addr, Context, ContextFutureSpawner, Handler, Message, WrapFuture,
};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use yahoo_finance_api as yahoo;

use crate::async_signals::{AsyncStockSignal, MaxPrice, MinPrice, PriceDifference, WindowedSMA};
use crate::constants::{CSV_FILE_NAME, CSV_HEADER, WINDOW_SIZE};

// ============================================================================
//
//                  [`QuoteRequestsMsg`] & [`FetchActor`]
//
// ============================================================================

/// The [`QuoteRequestsMsg`] message
///
/// It contains a `chunk` of symbols, and `from` and `to` fields.
///
/// It also contains a [`WriterActor`] address.
///
/// There is no expected response.
#[derive(Message)]
#[rtype(result = "()")]
pub struct QuoteRequestsMsg {
    pub chunk: Vec<String>,
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

impl FetchActor {
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
        // This function takes a single symbol.
        // The crate that we're using doesn't contain a function that works with a chunk of symbols.
        let yresponse = provider.get_quote_history(symbol, from, to).await?;

        let mut quotes = yresponse.quotes()?;

        let mut result = vec![];
        if !quotes.is_empty() {
            quotes.sort_by_cached_key(|k| k.timestamp);
            result = quotes.iter().map(|q| q.adjclose).collect();
        }

        Ok(result)
    }
}

/// The [`QuoteRequestsMsg`] message handler for the [`FetchActor`] actor
impl Handler<QuoteRequestsMsg> for FetchActor {
    type Result = ();

    /// The [`QuoteRequestsMsg`] message handler for the [`FetchActor`] actor
    ///
    /// Spawns a new [`ProcessorActor`] and sends it a [`SymbolsClosesMsg`] message.
    ///
    /// The message contains a hash map of `symbols` and associated `Vec<f64>` with closing prices for that symbol
    /// in case there was no error when fetching the data, or an empty vector in case of an error,
    /// in which case it prints the error message to `stderr`.
    ///
    /// So, in case of an API error for a symbol, when trying to fetch its data,
    /// we don't break the program but rather continue.
    fn handle(&mut self, msg: QuoteRequestsMsg, ctx: &mut Self::Context) -> Self::Result {
        let symbols = msg.chunk;
        let from = msg.from;
        let to = msg.to;
        let writer_address = msg.writer_address;

        let provider = yahoo::YahooConnector::new();

        let mut symbols_closes: HashMap<String, Vec<f64>> = HashMap::with_capacity(symbols.len());

        async move {
            for symbol in symbols {
                let closes = match FetchActor::fetch_closing_data(&symbol, from, to, &provider).await {
                    Ok(closes) => closes,
                    Err(err) => {
                        println!(
                            "There was an API error \"{}\" while fetching data for the symbol \"{}\"; \
                            skipping the symbol.",
                            err, symbol
                        );
                        vec![]
                    }
                };

                symbols_closes.insert(symbol, closes);
            }

            let symbols_closes_msg = SymbolsClosesMsg { symbols_closes, from, writer_address };

            // Spawn another Actor and send it the message.
            let proc_address = ProcessorActor.start();
            let _ = proc_address
                .send(symbols_closes_msg)
                .await
                .expect("Couldn't send a message to the ProcessorActor.");
        }
            .into_actor(self)
            .spawn(ctx);
    }
}

// ============================================================================
//
//                  [`SymbolsClosesMsg`] & [`ProcessorActor`]
//
// ============================================================================

/// The [`SymbolsClosesMsg`] message
///
/// It contains a hash map of `symbols` and associated `Vec<f64>` with closing prices for that symbol,
/// and the starting date and time `from` field.
///
/// It also contains a [`WriterActor`] address.
///
/// There is no expected response.
#[derive(Message)]
#[rtype(result = "()")]
struct SymbolsClosesMsg {
    pub symbols_closes: HashMap<String, Vec<f64>>,
    pub from: OffsetDateTime,
    pub writer_address: Addr<WriterActor>,
}

/// Actor for creating performance indicators from fetched stock data
struct ProcessorActor;

impl Actor for ProcessorActor {
    type Context = Context<Self>;
}

/// The [`SymbolsClosesMsg`] message handler for the [`ProcessorActor`] actor
impl Handler<SymbolsClosesMsg> for ProcessorActor {
    type Result = ();

    /// The [`SymbolsClosesMsg`] message handler for the [`ProcessorActor`] actor
    ///
    /// Sends a [`PerformanceIndicatorsRowsMsg`] message to the [`WriterActor`],
    /// whose address it gets from the [`SymbolsClosesMsg`] message.
    fn handle(&mut self, msg: SymbolsClosesMsg, ctx: &mut Self::Context) -> Self::Result {
        let symbols_closes = msg.symbols_closes;
        let from = msg.from;
        let writer_address = msg.writer_address;

        let from = OffsetDateTime::format(from, &Rfc3339).expect("Couldn't format 'from'.");

        async move {
            let mut rows: Vec<PerformanceIndicatorsRow> = Vec::with_capacity(symbols_closes.len());

            for symbol_closes in symbols_closes {
                let symbol = symbol_closes.0;
                let closes = symbol_closes.1;

                if !closes.is_empty() {
                    let min = MinPrice {};
                    let max = MaxPrice {};
                    let price_diff = PriceDifference {};
                    let n_window_sma = WindowedSMA {
                        window_size: WINDOW_SIZE,
                    };

                    let last_price = *closes.last().expect("Expected non-empty closes.");
                    let (_, pct_change) = price_diff.calculate(&closes).await.unwrap_or((0., 0.));
                    let pct_change = pct_change * 100.0;
                    let period_min: f64 = min.calculate(&closes).await.unwrap_or_default();
                    let period_max: f64 = max.calculate(&closes).await.unwrap_or_default();
                    let sma = n_window_sma.calculate(&closes).await.unwrap_or(vec![]);
                    let sma = *sma.last().unwrap_or(&0.0);

                    let row = PerformanceIndicatorsRow {
                        symbol: symbol.clone(),
                        last_price,
                        pct_change,
                        period_min,
                        period_max,
                        sma,
                    };

                    rows.push(row);

                    // A simple way to output CSV data
                    println!(
                        "{},{},${:.2},{:.2}%,${:.2},${:.2},${:.2}",
                        from, symbol, last_price, pct_change, period_min, period_max, sma,
                    );
                } else {
                    eprintln!("Got no data for the symbol \"{}\".", symbol);
                }
            }

            let perf_ind_msg = PerformanceIndicatorsRowsMsg { from, rows };

            // Send the message to the single writer actor.
            let _ = writer_address
                .send(perf_ind_msg)
                .await
                .expect("Couldn't send a message to the WriterActor.");
        }
        .into_actor(self)
        .spawn(ctx);
    }
}

// ============================================================================
//
//             [`PerformanceIndicatorsRowsMsg`] & [`WriterActor`]
//
// ============================================================================

/// A single row of calculated performance indicators for a symbol
struct PerformanceIndicatorsRow {
    pub symbol: String,
    pub last_price: f64,
    pub pct_change: f64,
    pub period_min: f64,
    pub period_max: f64,
    pub sma: f64,
}

/// The [`PerformanceIndicatorsRowsMsg`] message
///
/// It contains a `from` date and time field,
/// and calculated performance indicators for a chunk of symbols.
///
/// There is no expected response.
#[derive(Message)]
#[rtype(result = "()")]
struct PerformanceIndicatorsRowsMsg {
    pub from: String,
    pub rows: Vec<PerformanceIndicatorsRow>,
}

/// Actor for writing calculated performance indicators for fetched stock data into a CSV file
pub struct WriterActor {
    pub file_name: String,
    pub writer: Option<BufWriter<File>>,
}

impl WriterActor {
    pub fn new() -> Self {
        Self {
            file_name: CSV_FILE_NAME.to_string(),
            writer: None,
        }
    }
}

impl Actor for WriterActor {
    type Context = Context<Self>;
    // type Context = SyncContext<Self>; todo remove

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.set_mailbox_capacity(16); // Default capacity is 16 messages.
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

/// The [`PerformanceIndicatorsRowsMsg`] message handler for the [`WriterActor`] actor
impl Handler<PerformanceIndicatorsRowsMsg> for WriterActor {
    type Result = ();

    fn handle(
        &mut self,
        msg: PerformanceIndicatorsRowsMsg,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let from = msg.from;
        let rows = msg.rows;

        if let Some(file) = &mut self.writer {
            for row in rows {
                let _ = writeln!(
                    file,
                    "{},{},${:.2},{:.2}%,${:.2},${:.2},${:.2}",
                    from,
                    row.symbol,
                    row.last_price,
                    row.pct_change,
                    row.period_min,
                    row.period_max,
                    row.sma,
                );
            }

            file.flush().expect("Failed to flush to file. Data loss :/");
        }
    }
}
