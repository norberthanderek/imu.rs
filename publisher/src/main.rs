mod cli;

fn main() {
    let args = cli::PublisherArgs::parse();
    let logger = common::logging::setup_logger(args.log_level.clone());
    cli::PublisherArgs::print(&args, &logger);
}
