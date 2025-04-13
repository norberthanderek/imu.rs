# IMU Data Consumer
robust IMU sensor data consumer that connects socket managed by publisher

## Features
- Connects to Unix socket with configurable timeout
- Processes stream of Protocol Buffer encoded IMU data messages
- Computes orientation, velocity, and position using an integrated motion processor
- Comprehensive error handling for connection failures, timeouts, and malformed data
- Logs detailed motion state information for debugging and analysis