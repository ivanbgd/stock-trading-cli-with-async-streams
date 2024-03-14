use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::SendError;
use yahoo_finance_api as yahoo;

use crate::async_signals::{AsyncStockSignal, MaxPrice, MinPrice, PriceDifference, WindowedSMA};
use crate::constants::{CSV_FILE_NAME, CSV_HEADER, MPSC_CHANNEL_CAPACITY, WINDOW_SIZE};
use crate::types::{MsgErrorType, MsgResponseType};

//
//
// [`ActorMessage`], [`Actor`], [`ActorHandle`]
//
//

/// The [`ActorMessage`] enumeration
///
/// Supports two message types:
/// - QuoteRequestsMsg,
/// - SymbolsClosesMsg,
///
/// There is no expected response for any of the message types.
///
/// We could have an oneshot channel for sending the response back in general case.
/// It could be used for every message type.
/// We simply don't need it in our specific (custom) case.
pub enum ActorMessage {
    QuoteRequestsMsg {
        symbols: Vec<String>,
        from: OffsetDateTime,
        to: OffsetDateTime,
        writer_handle: WriterActorHandle,
    },
    SymbolsClosesMsg {
        symbols_closes: HashMap<String, Vec<f64>>,
        from: OffsetDateTime,
        writer_handle: WriterActorHandle,
    },
}

/// A universal (general) type of actor
///
/// It can receive and handle two message types.
///
/// It is not made public on purpose.
///
/// It can only be created through [`ActorHandle`], which is public.
struct Actor {
    receiver: mpsc::Receiver<ActorMessage>,
}

impl Actor {
    /// Create a new actor
    fn new(receiver: mpsc::Receiver<ActorMessage>) -> Self {
        Self { receiver }
    }

    /// Run the actor
    async fn run(&mut self) -> Result<MsgResponseType, MsgErrorType> {
        while let Some(msg) = self.receiver.recv().await {
            self.handle(msg).await?;
        }

        Ok(())
    }

    /// Handle the message
    async fn handle(&mut self, msg: ActorMessage) -> Result<MsgResponseType, MsgErrorType> {
        match msg {
            ActorMessage::QuoteRequestsMsg {
                symbols,
                from,
                to,
                writer_handle,
            } => {
                Self::handle_quote_requests_msg(symbols, from, to, writer_handle).await?;
            }
            ActorMessage::SymbolsClosesMsg {
                symbols_closes,
                from,
                writer_handle,
            } => {
                Self::handle_symbols_closes_msg(symbols_closes, from, writer_handle).await;
            }
        }

        Ok(())
    }

    /// The [`QuoteRequestsMsg`] message handler for the fetch [`Actor`] actor
    ///
    /// Spawns a new processor [`Actor`] and sends it a [`SymbolsClosesMsg`] message.
    ///
    /// The message contains a hash map of `symbols` and associated `Vec<f64>` with closing prices for that symbol
    /// in case there was no error when fetching the data, or an empty vector in case of an error,
    /// in which case it prints the error message to `stderr`.
    ///
    /// So, in case of an API error for a symbol, when trying to fetch its data,
    /// we don't break the program but rather continue.
    async fn handle_quote_requests_msg(
        symbols: Vec<String>,
        from: OffsetDateTime,
        to: OffsetDateTime,
        writer_handle: WriterActorHandle,
    ) -> Result<MsgResponseType, MsgErrorType> {
        let provider = yahoo::YahooConnector::new();

        let mut symbols_closes: HashMap<String, Vec<f64>> = HashMap::with_capacity(symbols.len());

        for symbol in symbols {
            let closes = match Self::fetch_closing_data(&symbol, from, to, &provider).await {
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

        let symbols_closes_msg = ActorMessage::SymbolsClosesMsg {
            symbols_closes,
            from,
            writer_handle,
        };

        // Spawn another Actor and send it the message.
        let actor_handle = ActorHandle::new();
        actor_handle.send(symbols_closes_msg).await?;

        Ok(())
    }

    /// The [`SymbolsClosesMsg`] message handler for the processor [`Actor`] actor
    ///
    /// Sends a [`PerformanceIndicatorsRowsMsg`] message to the [`WriterActor`],
    /// whose address it gets from the [`SymbolsClosesMsg`] message.
    async fn handle_symbols_closes_msg(
        symbols_closes: HashMap<String, Vec<f64>>,
        from: OffsetDateTime,
        writer_handle: WriterActorHandle,
    ) -> MsgResponseType {
        let from = OffsetDateTime::format(from, &Rfc3339).expect("Couldn't format 'from'.");

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
        match writer_handle.send(perf_ind_msg).await {
            Ok(response) => response,
            Err(err) => eprintln!("Couldn't send a message to the WriterActor: \"{:?}\"", err),
        }
    }

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

/// A handle for the [`Actor`]
///
/// Only the handle is public; the [`Actor`] isn't.
///
/// We can only create [`Actor`]s through the [`ActorHandle`].
///
/// It contains the `sender` field, which represents
/// a sender of the [`ActorMessage`] in an MPSC channel.
///
/// The handle is the sender, and the actor is the receiver
/// of a message in the channel.
///
/// We only create a single [`Actor`] instance in an [`ActorHandle`].
#[derive(Clone)]
pub struct ActorHandle {
    sender: mpsc::Sender<ActorMessage>, // TODO: Change to oneshot. Also change error type.
}

impl ActorHandle {
    /// Create a new [`ActorHandle`]
    ///
    /// This function creates a single [`Actor`] instance,
    /// and a MPSC channel for communicating to the actor.
    ///
    /// # Panics
    ///
    /// Panics if it can't run the actor.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(MPSC_CHANNEL_CAPACITY);
        let mut actor = Actor::new(receiver);
        tokio::spawn(async move {
            // actor.run().await.expect("Failed to run an actor.") // todo rm
            match actor.run().await {
                Ok(_) => (),
                Err(err) => eprintln!("Failed to run an actor: \"{:#?}\"", err),
            }
        });

        Self { sender }
    }

    /// Send a message to an [`Actor`] instance through the [`ActorHandle`]
    pub async fn send(&self, msg: ActorMessage) -> Result<MsgResponseType, MsgErrorType> {
        Ok(self.sender.send(msg).await?)
    }
}

//
//
// [`PerformanceIndicatorsRowsMsg`], [`WriterActor`], [`WriterActorHandle`]
//
//

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
///
/// We could have an oneshot channel for sending the response back in general case.
/// We simply don't need it in our specific (custom) case.
pub struct PerformanceIndicatorsRowsMsg {
    from: String,
    rows: Vec<PerformanceIndicatorsRow>,
}

/// Actor for writing calculated performance indicators for fetched stock data into a CSV file
///
/// It is not made public on purpose.
///
/// It can only be created through [`WriterActorHandle`], which is public.
struct WriterActor {
    receiver: mpsc::Receiver<PerformanceIndicatorsRowsMsg>,
    pub file_name: String,
    pub writer: Option<BufWriter<File>>,
}

impl WriterActor {
    /// Create a new [`WriterActor`]
    fn new(receiver: mpsc::Receiver<PerformanceIndicatorsRowsMsg>) -> Self {
        Self {
            receiver,
            file_name: CSV_FILE_NAME.to_string(),
            writer: None,
        }
    }

    /// Start the [`WriterActor`]
    ///
    /// This function is meant to be used directly in the [`WriterActorHandle`].
    async fn start(&mut self) -> Result<MsgResponseType, SendError<PerformanceIndicatorsRowsMsg>> {
        let mut file = File::create(&self.file_name)
            .unwrap_or_else(|_| panic!("Could not open target file \"{}\".", self.file_name));
        let _ = writeln!(&mut file, "{}", CSV_HEADER);
        self.writer = Some(BufWriter::new(file));
        println!("WriterActor is started.");

        Ok(self.run().await?)
    }

    /// Run the [`WriterActor`]
    ///
    /// This function is meant to be used indirectly - only through the [`WriterActor::start`] function
    async fn run(&mut self) -> Result<MsgResponseType, SendError<PerformanceIndicatorsRowsMsg>> {
        println!("WriterActor is running.");

        while let Some(msg) = self.receiver.recv().await {
            self.handle(msg);
        }

        Ok(())
    }

    /// Stop the [`WriterActor`]
    ///
    /// This function is meant to be called in the [`WriterActor`]'s destructor.
    fn stop(&mut self) {
        if let Some(writer) = &mut self.writer {
            writer
                .flush()
                .expect("Failed to flush writer. Data loss :(")
        };

        println!("WriterActor is flushed and properly stopped.");
    }

    /// The [`PerformanceIndicatorsRowsMsg`] message handler for the [`WriterActor`] actor
    fn handle(&mut self, msg: PerformanceIndicatorsRowsMsg) -> MsgResponseType {
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

impl Drop for WriterActor {
    fn drop(&mut self) {
        self.stop();
    }
}

/// A handle for the [`WriterActor`]
///
/// Only the handle is public; the [`WriterActor`] isn't.
///
/// We can only create [`WriterActor`]s through the [`WriterActorHandle`].
///
/// It contains the `sender` field, which represents
/// a sender of the [`PerformanceIndicatorsRowsMsg`] in an MPSC channel.
///
/// The handle is the sender, and the actor is the receiver
/// of a message in the channel.
///
/// We only create a single [`WriterActor`] instance in a [`WriterActorHandle`].
#[derive(Clone)]
pub struct WriterActorHandle {
    sender: mpsc::Sender<PerformanceIndicatorsRowsMsg>, // TODO: Change to oneshot. Also change error type.
}

impl WriterActorHandle {
    /// Create a new [`WriterActorHandle`]
    ///
    /// This function creates a single [`WriterActor`] instance,
    /// and a MPSC channel for communicating with the actor.
    ///
    /// It also starts (runs) the actor.
    ///
    /// # Panics
    ///
    /// Panics if it can't run the actor.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(MPSC_CHANNEL_CAPACITY);
        let mut actor = WriterActor::new(receiver);
        tokio::spawn(async move {
            // actor.start().await.expect("Failed to start a writer actor.") // todo rm
            match actor.start().await {
                Ok(_) => (),
                Err(err) => eprintln!("Failed to start a writer actor: \"{:#?}\"", err),
            }
        });

        Self { sender }
    }

    /// Send a message to an [`WriterActor`] instance through the [`WriterActorHandle`]
    pub async fn send(
        &self,
        msg: PerformanceIndicatorsRowsMsg,
    ) -> Result<MsgResponseType, SendError<PerformanceIndicatorsRowsMsg>> {
        Ok(self.sender.send(msg).await?)
    }
}
