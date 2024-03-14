pub const TICK_INTERVAL_SECS: u64 = 5;

pub const CHUNK_SIZE: usize = 5;

pub const NUM_THREADS: usize = 4;

pub const WINDOW_SIZE: usize = 30;

pub const CSV_HEADER: &str = "period start,symbol,price,change %,min,max,30d avg";

pub const CSV_FILE_NAME: &str = "output.csv";

pub const MPSC_CHANNEL_CAPACITY: usize = 1;
