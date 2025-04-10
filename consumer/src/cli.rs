use common::clap;
use common::cli_defaults::*;
use common::logging::LogLevel;
use common::slog;

#[derive(clap::Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct ConsumerArgs {
    #[arg(short, long, default_value = DEFAULT_SOCKET_PATH, value_parser = clap::value_parser!(std::path::PathBuf))]
    pub socket_path: std::path::PathBuf,

    #[arg(short, long, value_enum, default_value_t = DEFAULT_LOG_LEVEL, value_parser = clap::value_parser!(LogLevel))]
    pub log_level: LogLevel,

    #[arg(short, long, default_value = DEFAULT_TIMEOUT, value_parser = clap::value_parser!(u32).range(1..=60*1000))]
    pub timeout: u32,
}

impl ConsumerArgs {
    pub fn parse() -> Self {
        <Self as clap::Parser>::parse()
    }

    pub fn print(&self, logger: &slog::Logger) {
        slog::info!(logger, "Log level: {:?}", self.log_level);
        slog::info!(logger, "Socket path: {:?}", self.socket_path);
        slog::info!(logger, "Timeout: {:?}ms", self.timeout);
    }
}
