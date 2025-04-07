# IMU Driver
publisher -> consumer IPC example 

![](https://shields.io/badge/-Rust-3776AB?style=flat&logo=rust)

## Dependencies
Rust + Cargo, both installed via [rustup](https://www.rust-lang.org/tools/install)

## Build & run
```sh
cargo build
```

## Docker
To ensure compatibility with Ubuntu, whole environment has been contenerized.

### VSCode
`> Dev Containers: Reopen in container`

### Terminal
```sh
./docker/run.sh # start or attach to existing container
./docker/stop.sh # stop currently running container
```