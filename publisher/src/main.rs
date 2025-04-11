mod cli;
mod imu_emulator;
mod publisher;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = cli::PublisherArgs::parse();
    let logger = common::logging::setup_logger(args.log_level.clone());
    cli::PublisherArgs::print(&args, &logger);

    publisher::Publisher::new(args.socket_path, args.frequency, logger.clone())
        .run()
        .await
}
