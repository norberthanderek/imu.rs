FROM ubuntu:24.04

ARG USERNAME=dev
ARG DOCKER_UID
ARG DOCKER_GID

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    build-essential \
    sudo \
    curl \
    pkg-config \
    libssl-dev \
    git \
    neovim \
    protobuf-compiler \
    libprotobuf-dev \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /etc/sudoers.d \
    && getent group $DOCKER_GID || groupadd --gid $DOCKER_GID $USERNAME \
    && useradd --uid $DOCKER_UID --gid $DOCKER_GID -m $USERNAME \
    && echo $USERNAME ALL=\(root\) NOPASSWD:ALL > /etc/sudoers.d/$USERNAME \
    && chmod 0440 /etc/sudoers.d/$USERNAME

USER $USERNAME
WORKDIR /home/$USERNAME/imu.rs

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/home/dev/.cargo/bin:${PATH}"

RUN rustup component add rustfmt clippy rust-analyzer
RUN cargo install cargo-edit cargo-watch cargo-expand

