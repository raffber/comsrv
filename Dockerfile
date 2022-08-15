FROM lukemathwalker/cargo-chef:latest-rust-1.63-buster as chef
WORKDIR /workspace
RUN cargo install cargo-xwin && \
    rustup target add x86_64-pc-windows-msvc && \
    cargo install xwin && \
    mkdir /cache && \
    xwin --accept-license --cache-dir /cache/xwin splat

FROM chef as planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder

RUN apt-get update && \
    apt-get install --yes build-essential libudev-dev udev pkg-config mingw-w64 libclang-dev libusb-1.0-0-dev curl sudo clang llvm-dev

COPY --from=planner /workspace/recipe.json recipe.json
RUN cargo chef cook --release  --recipe-path recipe.json && mv target /cache/linux-release-cache
