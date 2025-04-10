mod cli;
mod imu_emulator;

fn main() {
    let args = cli::PublisherArgs::parse();
    let logger = common::logging::setup_logger(args.log_level.clone());
    cli::PublisherArgs::print(&args, &logger);

    let imu_data = common::proto::ImuData {
        x_acc: 0.0,
        y_acc: 0.0,
        z_acc: 0.0,
        timestamp_acc: 0,
        x_gyro: 0,
        y_gyro: 0,
        z_gyro: 0,
        timestamp_gyro: 0,
        x_mag: 0.0,
        y_mag: 0.0,
        z_mag: 0.0,
        timestamp_mag: 0,
    };

    common::slog::info!(logger, "IMU Data: {:?}", imu_data);
}
