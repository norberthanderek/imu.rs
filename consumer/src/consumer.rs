use crate::motion::MotionProcessor;
use common::prost::Message;
use common::proto::ImuData;
use common::slog::{Logger, error, info, warn};
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::net::UnixStream;
use tokio::time::timeout;

#[derive(Debug)]
pub struct Consumer {
    socket_path: PathBuf,
    timeout: Duration,
    logger: Logger,
    motion_processor: MotionProcessor,
}

impl Consumer {
    pub fn new(socket_path: PathBuf, timeout: u32, logger: Logger) -> Self {
        let motion_processor = MotionProcessor::new(logger.clone());
        let timeout = Duration::from_secs(timeout.into());
        Self {
            socket_path,
            timeout,
            logger,
            motion_processor,
        }
    }

    pub async fn run(&mut self) -> std::io::Result<()> {
        info!(self.logger, "Attempting to connect to socket"; "path" => %self.socket_path.display(), "timeout" => ?self.timeout);

        let stream = match timeout(self.timeout, UnixStream::connect(&self.socket_path)).await {
            Ok(Ok(stream)) => {
                info!(self.logger, "Successfully connected to socket"; "path" => %self.socket_path.display());
                stream
            }
            Ok(Err(e)) => {
                error!(self.logger, "Failed to connect to socket"; "path" => %self.socket_path.display(), "error" => %e);
                return Err(e);
            }
            Err(_) => {
                error!(self.logger, "Connection attempt timed out"; "path" => %self.socket_path.display(), "timeout" => ?self.timeout);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "connection timed out",
                ));
            }
        };

        let mut reader = BufReader::new(stream);
        let mut buffer = Vec::new();

        loop {
            let message_len = match reader.read_u32().await {
                Ok(len) => len as usize,
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    info!(self.logger, "Connection closed cleanly (EOF)");
                    break Ok(());
                }
                Err(e) => {
                    error!(self.logger, "Failed to read message length"; "error" => %e);
                    break Err(e);
                }
            };

            if message_len == 0 {
                warn!(self.logger, "Received message with length 0, skipping.");
                continue;
            }

            buffer.resize(message_len, 0);
            if let Err(e) = reader.read_exact(&mut buffer).await {
                error!(self.logger, "Failed to read message body"; "expected_len" => message_len, "error" => %e);
                break Err(e);
            }

            match ImuData::decode(buffer.as_slice()) {
                Ok(imu_data) => {
                    let state = self.motion_processor.process(&imu_data);

                    info!(
                        self.logger,
                        "Pos: [{:+.3},{:+.3},{:+.3}]m | Vel: [{:+.3},{:+.3},{:+.3}]m/s | Orient: [{:+.3},{:+.3},{:+.3},{:+.3}]quat",
                        state.position.x,
                        state.position.y,
                        state.position.z,
                        state.velocity.x,
                        state.velocity.y,
                        state.velocity.z,
                        state.orientation.scalar(),
                        state.orientation.vector().x,
                        state.orientation.vector().y,
                        state.orientation.vector().z
                    );
                }
                Err(e) => {
                    warn!(self.logger, "Failed to decode ImuData"; "error" => %e, "bytes_read" => message_len);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::prost::Message;
    use common::proto::ImuData;
    use common::slog::o;
    use std::fs;
    use std::io;
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixListener;

    fn setup_socket_path(test_name: &str) -> PathBuf {
        let socket_dir = PathBuf::from("/tmp");
        fs::create_dir_all(&socket_dir).expect("Failed to create /tmp for test sockets");
        let socket_path = socket_dir.join(format!("test_imu_consumer_{}.sock", test_name));
        if socket_path.exists() {
            fs::remove_file(&socket_path).unwrap_or_else(|e| {
                panic!(
                    "Pre-test cleanup: Failed to remove existing socket file '{}': {}",
                    socket_path.display(),
                    e
                )
            });
        }
        socket_path
    }

    fn cleanup_socket(socket_path: &PathBuf) {
        if socket_path.exists() {
            fs::remove_file(socket_path).unwrap_or_else(|e| {
                eprintln!(
                    "WARN: Post-test cleanup failed for socket file '{}': {}",
                    socket_path.display(),
                    e
                );
            });
        }
    }

    fn create_logger() -> common::slog::Logger {
        common::slog::Logger::root(common::slog::Discard, o!())
    }

    fn spawn_consumer_task(
        socket_path: PathBuf,
        timeout_secs: u32,
        logger: common::slog::Logger,
    ) -> tokio::task::JoinHandle<std::io::Result<()>> {
        tokio::spawn(async move {
            let mut consumer = Consumer::new(socket_path, timeout_secs, logger);
            consumer.run().await
        })
    }

    fn create_test_imu_data(timestamp: u32) -> ImuData {
        ImuData {
            x_acc: 1.0,
            y_acc: 2.0,
            z_acc: 3.0,
            timestamp_acc: timestamp,
            x_gyro: 1,
            y_gyro: 2,
            z_gyro: 3,
            timestamp_gyro: timestamp,
            x_mag: 0.01,
            y_mag: 0.02,
            z_mag: 0.03,
            timestamp_mag: timestamp,
        }
    }

    async fn send_message(stream: &mut UnixStream, msg: &ImuData) -> io::Result<()> {
        let mut buf = Vec::new();
        msg.encode(&mut buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let len = buf.len() as u32;

        stream.write_u32(len).await?;
        stream.write_all(&buf).await?;
        stream.flush().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_consumer_connect_and_receive_valid_data() {
        let socket_path = setup_socket_path("connect_receive_valid");
        let logger = create_logger();

        let listener = UnixListener::bind(&socket_path).expect("Failed to bind listener");
        let consumer_handle = spawn_consumer_task(socket_path.clone(), 5, logger.clone());

        let (mut stream, _) = listener
            .accept()
            .await
            .expect("Failed to accept connection");

        let msg1 = create_test_imu_data(100);
        let msg2 = create_test_imu_data(200);

        send_message(&mut stream, &msg1)
            .await
            .expect("Failed to send msg1");
        info!(logger, "Test server sent message 1");
        tokio::time::sleep(Duration::from_millis(50)).await;

        send_message(&mut stream, &msg2)
            .await
            .expect("Failed to send msg2");
        info!(logger, "Test server sent message 2");
        tokio::time::sleep(Duration::from_millis(50)).await;

        drop(stream);
        info!(logger, "Test server closed connection");

        let result = tokio::time::timeout(Duration::from_secs(1), consumer_handle).await;

        match result {
            Ok(Ok(Ok(()))) => {
                info!(logger, "Consumer task finished successfully as expected.");
            }
            Ok(Ok(Err(e))) => {
                panic!("Consumer task finished with an unexpected IO error: {}", e);
            }
            Ok(Err(join_err)) => {
                panic!("Consumer task panicked or was cancelled: {}", join_err);
            }
            Err(_) => {
                panic!("Consumer task timed out waiting for completion after connection close");
            }
        }

        cleanup_socket(&socket_path);
    }

    #[tokio::test]
    async fn test_consumer_handles_eof_correctly() {
        let socket_path = setup_socket_path("handle_eof");
        let logger = create_logger();

        let listener = UnixListener::bind(&socket_path).expect("Failed to bind listener");
        let consumer_handle = spawn_consumer_task(socket_path.clone(), 5, logger.clone());

        let (mut stream, _) = listener
            .accept()
            .await
            .expect("Failed to accept connection");

        let msg1 = create_test_imu_data(100);
        send_message(&mut stream, &msg1)
            .await
            .expect("Failed to send msg1");
        info!(logger, "Test server sent one message");
        tokio::time::sleep(Duration::from_millis(100)).await;

        drop(stream);
        info!(logger, "Test server closed connection");

        let result = tokio::time::timeout(Duration::from_secs(1), consumer_handle).await;

        match result {
            Ok(Ok(Ok(()))) => {
                info!(
                    logger,
                    "Consumer task finished successfully on EOF as expected."
                );
            }
            Ok(Ok(Err(e))) => {
                panic!(
                    "Consumer task finished with an unexpected IO error on EOF: {}",
                    e
                );
            }
            Ok(Err(join_err)) => {
                panic!(
                    "Consumer task panicked or was cancelled on EOF: {}",
                    join_err
                );
            }
            Err(_) => {
                panic!("Consumer task timed out waiting for completion after EOF");
            }
        }

        cleanup_socket(&socket_path);
    }

    #[tokio::test]
    async fn test_consumer_connection_fails_before_timeout() {
        let socket_path = setup_socket_path("connection_fail_quick");
        let logger = create_logger();

        let mut consumer = Consumer::new(socket_path.clone(), 5, logger.clone());
        let result = consumer.run().await;

        assert!(
            result.is_err(),
            "Consumer::run should return an error when connection fails"
        );

        if let Err(e) = result {
            assert!(
                matches!(
                    e.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                ),
                "Expected NotFound or ConnectionRefused when socket doesn't exist/isn't listening, but got: {:?}",
                e.kind()
            );
            info!(
                logger,
                "Consumer correctly failed to connect before timeout as expected: {}", e
            );
        } else {
            panic!("Consumer::run succeeded unexpectedly when connection should fail");
        }

        cleanup_socket(&socket_path);
    }

    #[tokio::test]
    async fn test_consumer_connection_refused() {
        let socket_path = setup_socket_path("connection_refused");
        let logger = create_logger();

        let mut consumer = Consumer::new(socket_path.clone(), 5, logger.clone());

        let result = consumer.run().await;
        assert!(
            result.is_err(),
            "Consumer::run should return an error on connection refused/failed"
        );

        if let Err(e) = result {
            info!(logger, "Consumer failed to connect as expected: {}", e);
            assert!(
                matches!(
                    e.kind(),
                    std::io::ErrorKind::ConnectionRefused
                        | std::io::ErrorKind::NotFound
                        | std::io::ErrorKind::TimedOut
                ),
                "Expected connection failure error kind, got {:?}",
                e.kind()
            );
        }

        cleanup_socket(&socket_path);
    }

    #[tokio::test]
    async fn test_consumer_handles_zero_length_message() {
        let socket_path = setup_socket_path("zero_length");
        let logger = create_logger();

        let listener = UnixListener::bind(&socket_path).expect("Failed to bind listener");
        let consumer_handle = spawn_consumer_task(socket_path.clone(), 5, logger.clone());

        let (mut stream, _) = listener
            .accept()
            .await
            .expect("Failed to accept connection");

        stream
            .write_u32(0)
            .await
            .expect("Failed to send zero length");
        info!(logger, "Test server sent zero length message");
        tokio::time::sleep(Duration::from_millis(50)).await;

        let msg = create_test_imu_data(300);
        send_message(&mut stream, &msg)
            .await
            .expect("Failed to send valid message after zero");
        info!(logger, "Test server sent valid message after zero");
        tokio::time::sleep(Duration::from_millis(50)).await;

        drop(stream);
        info!(logger, "Test server closed connection");

        let result = tokio::time::timeout(Duration::from_secs(1), consumer_handle).await;

        match result {
            Ok(Ok(Ok(()))) => {
                info!(
                    logger,
                    "Consumer task finished successfully after zero-length message and EOF."
                );
            }
            Ok(Ok(Err(e))) => {
                panic!(
                    "Consumer task finished with unexpected IO error after zero-length message: {}",
                    e
                );
            }
            Ok(Err(join_err)) => {
                panic!(
                    "Consumer task panicked or was cancelled after zero-length message: {}",
                    join_err
                );
            }
            Err(_) => {
                panic!("Consumer task timed out waiting for completion after zero-length message");
            }
        }

        cleanup_socket(&socket_path);
    }

    #[tokio::test]
    async fn test_consumer_handles_invalid_protobuf_data() {
        let socket_path = setup_socket_path("invalid_protobuf");
        let logger = create_logger();

        let listener = UnixListener::bind(&socket_path).expect("Failed to bind listener");
        let consumer_handle = spawn_consumer_task(socket_path.clone(), 5, logger.clone());

        let (mut stream, _) = listener
            .accept()
            .await
            .expect("Failed to accept connection");

        let invalid_data = b"this is not protobuf data";
        let len = invalid_data.len() as u32;
        stream
            .write_u32(len)
            .await
            .expect("Failed to send length for invalid data");
        stream
            .write_all(invalid_data)
            .await
            .expect("Failed to send invalid data");
        stream.flush().await.expect("Failed to flush invalid data");
        info!(logger, "Test server sent invalid protobuf data");
        tokio::time::sleep(Duration::from_millis(100)).await;

        let valid_msg = create_test_imu_data(400);
        send_message(&mut stream, &valid_msg)
            .await
            .expect("Failed to send valid message after invalid");
        info!(logger, "Test server sent valid message after invalid");
        tokio::time::sleep(Duration::from_millis(100)).await;

        drop(stream);
        info!(logger, "Test server closed connection");

        let result = tokio::time::timeout(Duration::from_secs(1), consumer_handle).await;

        match result {
            Ok(Ok(Ok(()))) => {
                info!(
                    logger,
                    "Consumer task finished successfully after invalid protobuf and EOF."
                );
            }
            Ok(Ok(Err(e))) => {
                panic!(
                    "Consumer task finished with unexpected IO error after invalid protobuf: {}",
                    e
                );
            }
            Ok(Err(join_err)) => {
                panic!(
                    "Consumer task panicked or was cancelled after invalid protobuf: {}",
                    join_err
                );
            }
            Err(_) => {
                panic!("Consumer task timed out waiting for completion after invalid protobuf");
            }
        }

        cleanup_socket(&socket_path);
    }
}
