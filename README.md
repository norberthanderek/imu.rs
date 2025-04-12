# IMU Driver
publisher -> consumer IPC example 

![](https://shields.io/badge/-Rust-3776AB?style=flat&logo=rust)

## Dependencies
### Rust + Cargo
both installed via [rustup](https://www.rust-lang.org/tools/install)
### Protobuf compiler
```sh
apt update && apt install -y protobuf-compiler libprotobuf-dev
```

## Build
```sh
cargo build --release
```

## Run
```sh
./target/release/publisher # --help
# on separate shell
./target/release/consumer # --help
```

## Test
```sh
cargo test
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

## Technology rationale
- rust: although C++ is my day-to-day language and I have experience building successful tools in Python, I chose Rust for this task.
I opted for it due to its strong performance and memory safety guarantees, and mostly because I'm enthusiastic about applying its capabilities.
- protobuf: leveraging rich and performant library to ensure top notch communication between services.
- slog: fast & easy logging, highly extendable due to it's structured nature
- tokio: industry-standard asynchronous runtime for fast, scalable, and non-blocking application logic.
- clap: easy CLI setup
- nalgebra, rand, approx: simply reliable

## Real-time
As my private workstation is a macbook (because it's unix-like and has propiratary software support)
this whole project was developed in a Ubuntu devcontainer.\
Linux containers need host with Linux kernel supporting RT to be able to support it themselves.\
My host is not Linux, so RT is not integrated.

There are tricks like emulation + ssh or dualboot but it exceeds scope and purpose of this project.