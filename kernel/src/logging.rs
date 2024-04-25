use log::{self, Level, LevelFilter, Log, Metadata, Record};

struct SimpleLogger;

static LOGGER: SimpleLogger = SimpleLogger;

impl Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }
    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let color = match record.level() {
            Level::Error => 31, // Red
            Level::Warn => 93,  // Yellow
            Level::Info => 34,  // Blue
            Level::Debug => 32, // Green
            Level::Trace => 36, // Cyan
        };
        println!(
            "\u{1B}[{}m[{:>5}] {}:{} {}\u{1B}[0m",
            color,
            record.level(),
            record.file().unwrap(),
            record.line().unwrap(),
            record.args(),
        );
    }
    fn flush(&self) {
        unimplemented!("logging buffer is not immplemented");
    }
}

/// Initializate kernel logger, filtering by log level.
pub fn init() {
    log::set_logger(&LOGGER).unwrap();

    log::set_max_level(LevelFilter::Debug);

    // log::set_max_level(match option_env!("RUST_LOG") {
    //     Some(log_level) => match log_level {
    //         "ERROR" | "error" => LevelFilter::Error,
    //         "WARN" | "warn" => LevelFilter::Warn,
    //         "INFO" | "info" => LevelFilter::Info,
    //         "DEBUG" | "debug" => LevelFilter::Debug,
    //         "TRACE" | "trace" => LevelFilter::Trace,
    //         _ => LevelFilter::Off,
    //     },
    //     None => LevelFilter::Info,
    // });
}
