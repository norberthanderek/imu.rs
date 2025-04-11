mod cli;
mod consumer;
mod motion;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = cli::ConsumerArgs::parse();
    let logger = common::logging::setup_logger(args.log_level.clone());
    cli::ConsumerArgs::print(&args, &logger);

    consumer::Consumer::new(args.socket_path, args.timeout, logger.clone())
        .run()
        .await
}
