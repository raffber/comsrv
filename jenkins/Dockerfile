FROM ubuntu:20.04

ENV TZ=Europe/Zurich RUSTUP_HOME=/rust CARGO_HOME=/cargo PATH=/cargo/bin:/rust/bin:$PATH HOME=/home

COPY docker-install.sh /root
RUN /root/docker-install.sh

RUN apt-get install --yes sudo 