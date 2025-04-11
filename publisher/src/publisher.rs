use super::imu_emulator;

use common::prost::Message;
use common::slog::{Logger, debug, error, info, warn};

use tokio::io::AsyncWriteExt;
use tokio::net::{UnixListener, UnixStream};
use tokio::time::{Duration, interval};

use std::fs;
use std::io;
use std::path::PathBuf;

pub struct Publisher {
    socket_path: PathBuf,
    frequency_hz: u32,
    logger: Logger,
    emulator: imu_emulator::ImuEmulator,
}

impl Publisher {
    pub fn new(socket_path: PathBuf, frequency_hz: u32, logger: Logger) -> Self {
        Publisher {
            socket_path,
            frequency_hz,
            logger,
            emulator: imu_emulator::ImuEmulator::new(),
        }
    }

    fn io_error<E: std::fmt::Display>(kind: io::ErrorKind, e: E) -> io::Error {
        io::Error::new(kind, e.to_string())
    }

    async fn ensure_socket_path(&self) -> io::Result<()> {
        let path = self.socket_path.as_path();

        // Clean up existing socket if needed
        if path.exists() {
            warn!(self.logger, "Socket file already exists. Removing it.");
            fs::remove_file(path).map_err(|e| {
                error!(self.logger, "Failed to remove existing socket: {}", e);
                e
            })?;
        }

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                info!(self.logger, "Creating parent directories");
                fs::create_dir_all(parent).map_err(|e| {
                    error!(self.logger, "Failed to create directories: {}", e);
                    e
                })?;
            }
        }

        Ok(())
    }

    async fn setup_socket(&self) -> io::Result<UnixListener> {
        self.ensure_socket_path().await?;

        info!(
            self.logger,
            "Creating socket at {}",
            self.socket_path.display()
        );

        match UnixListener::bind(&self.socket_path) {
            Ok(listener) => {
                info!(self.logger, "Socket created successfully");
                Ok(listener)
            }
            Err(e) => {
                error!(self.logger, "Failed to create socket: {}", e);
                Err(e)
            }
        }
    }

    async fn wait_for_consumer(&self, listener: &UnixListener) -> io::Result<UnixStream> {
        info!(self.logger, "Waiting for consumer to connect...");
        match listener.accept().await {
            Ok((stream, _addr)) => {
                info!(self.logger, "Consumer connected");
                Ok(stream)
            }
            Err(e) => {
                error!(self.logger, "Failed to accept connection: {}", e);
                Err(e)
            }
        }
    }

    async fn send_message(
        &self,
        stream: &mut UnixStream,
        data: &common::proto::ImuData,
    ) -> io::Result<()> {
        let mut buf = Vec::with_capacity(data.encoded_len());
        data.encode(&mut buf)
            .map_err(|e| Self::io_error(io::ErrorKind::Other, format!("Encoding error: {}", e)))?;

        let len_bytes = (buf.len() as u32).to_be_bytes();
        stream.write_all(&len_bytes).await?;
        stream.write_all(&buf).await?;
        stream.flush().await?;

        Ok(())
    }

    async fn publish_data(&mut self, mut stream: UnixStream) -> io::Result<()> {
        info!(
            self.logger,
            "Starting to publish data at {} Hz", self.frequency_hz
        );

        let interval_duration =
            Duration::from_micros((1_000_000.0 / self.frequency_hz as f64) as u64);
        let mut interval_timer = interval(interval_duration);

        let mut consecutive_errors = 0;
        const MAX_CONSECUTIVE_ERRORS: u32 = 5;

        loop {
            interval_timer.tick().await;

            let imu_data = self.emulator.generate_data();
            debug!(self.logger, "Generated IMU data: {:?}", &imu_data);

            match self.send_message(&mut stream, &imu_data).await {
                Ok(_) => {
                    consecutive_errors = 0;
                }
                Err(e) => {
                    error!(self.logger, "Failed to send message: {}", e);
                    consecutive_errors += 1;

                    if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                        error!(
                            self.logger,
                            "Too many consecutive errors, stopping publisher"
                        );
                        return Err(Self::io_error(
                            io::ErrorKind::BrokenPipe,
                            "Connection broken",
                        ));
                    }

                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    pub async fn run(&mut self) -> io::Result<()> {
        let listener = self.setup_socket().await?;

        loop {
            let stream = match self.wait_for_consumer(&listener).await {
                Ok(stream) => stream,
                Err(e) => {
                    error!(self.logger, "Failed to accept connection: {}", e);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            };

            match self.publish_data(stream).await {
                Ok(_) => {
                    info!(self.logger, "Publisher finished normally");
                    break;
                }
                Err(e) => {
                    if e.kind() == io::ErrorKind::BrokenPipe {
                        info!(
                            self.logger,
                            "Consumer disconnected, waiting for new connection"
                        );
                    } else {
                        error!(self.logger, "Publisher error: {}", e);
                        return Err(e);
                    }
                }
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::slog::o;
    use std::time::Duration;
    use tokio::io::AsyncReadExt;
    use tokio::net::UnixStream;

    async fn read_imu_message(stream: &mut UnixStream) -> io::Result<common::proto::ImuData> {
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await.map_err(|e| {
            io::Error::new(e.kind(), format!("Failed to read message length: {}", e))
        })?;

        let msg_len = u32::from_be_bytes(len_buf);
        let mut msg_buf = vec![0u8; msg_len as usize];

        stream
            .read_exact(&mut msg_buf)
            .await
            .map_err(|e| io::Error::new(e.kind(), format!("Failed to read message data: {}", e)))?;

        common::proto::ImuData::decode(&msg_buf[..]).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to decode message: {}", e),
            )
        })
    }

    fn setup_socket_path(test_name: &str) -> PathBuf {
        let socket_path = PathBuf::from(format!("/tmp/test_imu_{}", test_name));
        if socket_path.exists() {
            fs::remove_file(&socket_path).unwrap_or_else(|_| {
                panic!(
                    "Failed to remove existing socket file: {}",
                    socket_path.display()
                )
            });
        }
        socket_path
    }

    fn cleanup_socket(socket_path: PathBuf) {
        if socket_path.exists() {
            fs::remove_file(&socket_path).unwrap_or_else(|_| {
                panic!("Failed to clean up socket file: {}", socket_path.display())
            });
        }
    }

    fn create_logger() -> common::slog::Logger {
        common::slog::Logger::root(common::slog::Discard, o!())
    }

    fn spawn_publisher(
        socket_path: PathBuf,
        frequency_hz: u32,
        logger: common::slog::Logger,
        retry_on_error: bool,
    ) {
        std::thread::spawn({
            let socket_path = socket_path.clone();
            let logger = logger.clone();
            move || {
                let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
                rt.block_on(async {
                    if retry_on_error {
                        loop {
                            let mut publisher =
                                Publisher::new(socket_path.clone(), frequency_hz, logger.clone());
                            if (publisher.run().await).is_err() {
                                std::thread::sleep(std::time::Duration::from_millis(100));
                            } else {
                                break;
                            }
                        }
                    } else {
                        let mut publisher = Publisher::new(socket_path, frequency_hz, logger);
                        let _ = publisher.run().await;
                    }
                });
            }
        });
    }

    async fn connect_to_publisher(socket_path: &PathBuf, delay_ms: u64) -> io::Result<UnixStream> {
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        UnixStream::connect(socket_path).await.map_err(|e| {
            io::Error::new(
                e.kind(),
                format!(
                    "Failed to connect to publisher at {}: {}",
                    socket_path.display(),
                    e
                ),
            )
        })
    }

    #[tokio::test]
    async fn test_basic_publisher_functionality() {
        let socket_path = setup_socket_path("ipc_socket");
        let logger = create_logger();
        spawn_publisher(socket_path.clone(), 500, logger, false);

        let mut stream = connect_to_publisher(&socket_path, 200)
            .await
            .expect("Failed to connect to publisher");

        let mut message_count = 0;
        let mut last_timestamp = 0;

        for _ in 0..3 {
            match read_imu_message(&mut stream).await {
                Ok(data) => {
                    assert!(
                        data.timestamp_acc >= last_timestamp,
                        "Timestamp should be monotonically increasing"
                    );
                    last_timestamp = data.timestamp_acc;
                    message_count += 1;
                }
                Err(e) => {
                    panic!("Failed to read IMU data: {}", e);
                }
            }
        }

        assert_eq!(message_count, 3, "Should receive three messages");
        cleanup_socket(socket_path);
    }

    #[tokio::test]
    async fn test_slow_consumer() {
        let socket_path = setup_socket_path("slow_consumer");
        let logger = create_logger();

        const FAST_PUBLISHER_HZ: u32 = 1000;
        const SLOW_CONSUMER_HZ: u64 = 20;

        spawn_publisher(socket_path.clone(), FAST_PUBLISHER_HZ, logger, false);

        let mut stream = connect_to_publisher(&socket_path, 200)
            .await
            .expect("Failed to connect to publisher");

        for _ in 0..5 {
            let data = read_imu_message(&mut stream)
                .await
                .expect("Failed to read IMU message");

            assert!(data.timestamp_acc > 0, "Timestamp should be greater than 0");
            tokio::time::sleep(Duration::from_millis(1000 / SLOW_CONSUMER_HZ)).await;
        }

        cleanup_socket(socket_path);
    }

    #[tokio::test]
    async fn test_connection_drops_and_reconnects() {
        let socket_path = setup_socket_path("reconnect");
        let logger = create_logger();
        spawn_publisher(socket_path.clone(), 500, logger, true);

        // First connection
        {
            let mut stream = connect_to_publisher(&socket_path, 200)
                .await
                .expect("Failed to connect to publisher on first attempt");

            for _ in 0..2 {
                let data = read_imu_message(&mut stream)
                    .await
                    .expect("Failed to read IMU message on first connection");

                assert!(data.timestamp_acc > 0, "Timestamp should be greater than 0");
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;

        // Reconnection
        {
            let mut stream = connect_to_publisher(&socket_path, 0)
                .await
                .expect("Failed to reconnect to publisher");

            for _ in 0..2 {
                let data = read_imu_message(&mut stream)
                    .await
                    .expect("Failed to read IMU message on reconnection");

                assert!(data.timestamp_acc > 0, "Timestamp should be greater than 0");
            }
        }

        cleanup_socket(socket_path);
    }
}
