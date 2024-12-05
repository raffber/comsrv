FROM rust:latest-rust-1.83-bookworm as builder


RUN apt-get update && apt-get install -y \
    libudev-dev libclang-dev \
    && rm -rf /var/lib/apt

WORKDIR /workspace
ADD --chown=0:0 . /workspace/

RUN cargo build --release

FROM debian:bookworm-slim

COPY --from=builder /workspace/target/release/comsrv /usr/bin/comsrv

ENTRYPOINT [ "/usr/bin/comsrv" ]
CMD ["-p", "5902", "-h", "5903", "-v"]
