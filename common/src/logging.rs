use clap::ValueEnum;
use slog::{Drain, Level, Logger, o};
use slog_async::Async;
use slog_term::{FullFormat, TermDecorator};

#[derive(ValueEnum, Clone, Debug)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
    Trace,
}

impl From<LogLevel> for Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => Level::Error,
            LogLevel::Warning => Level::Warning,
            LogLevel::Info => Level::Info,
            LogLevel::Debug => Level::Debug,
            LogLevel::Trace => Level::Trace,
        }
    }
}

pub fn setup_logger(log_level: LogLevel) -> Logger {
    let decorator = TermDecorator::new().build();
    let drain = FullFormat::new(decorator).build().fuse();
    let drain = Async::new(drain).build().fuse();
    let drain = slog::LevelFilter::new(drain, log_level.into()).fuse();

    Logger::root(drain, o!())
}
