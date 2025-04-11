mod cli;
mod motion;

fn main() {
    let args = cli::ConsumerArgs::parse();
    let logger = common::logging::setup_logger(args.log_level.clone());
    cli::ConsumerArgs::print(&args, &logger);
}
