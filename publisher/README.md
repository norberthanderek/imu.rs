# IMU Data Publisher
IMU sensor emulator and publisher that creates and utilizes its own UNIX socket

## Features
- Creates and manages Unix socket connections for IPC
- Publishes Protocol Buffer encoded IMU data at configurable frequency
- Handles consumer connections, disconnections, and reconnections
- Implements proper socket cleanup and directory management
- Provides reliable error handling with graceful recovery