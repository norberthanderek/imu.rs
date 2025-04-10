use crate::logging::LogLevel;

pub const DEFAULT_LOG_LEVEL: LogLevel = LogLevel::Info;
pub const DEFAULT_SOCKET_PATH: &str = "/tmp/imu-ipc.sock";
pub const DEFAULT_FREQUENCY: &str = "500"; // Hz
pub const DEFAULT_TIMEOUT: &str = "1000"; // ms
