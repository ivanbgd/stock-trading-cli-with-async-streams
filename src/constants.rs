pub const TICK_INTERVAL_SECS: u64 = 5;
pub const SHUTDOWN_INTERVAL_SECS: u64 = 2;

pub const CHUNK_SIZE: usize = 5;

pub const NUM_THREADS: usize = 4;

pub const WINDOW_SIZE: usize = 30;

pub const CSV_FILE_PATH: &str = "./output.csv";
pub const CSV_HEADER: &str = "period start,symbol,price,change %,min,max,30d avg";

pub const ACTOR_CHANNEL_CAPACITY: usize = 1;
pub const SHUTDOWN_CHANNEL_CAPACITY: usize = 1;

pub const WEB_SERVER_ADDRESS: &str = "127.0.0.1:3000";

/// The tail buffer's capacity in terms of the number of batches it can hold
pub const TAIL_BUFFER_SIZE: usize = 10;
