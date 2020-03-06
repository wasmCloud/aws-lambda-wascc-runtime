# Build image for building with Rust on Amazon Linux, suitable for custom AWS Lambda runtimes.

FROM amazonlinux:2018.03.0.20191219.0
LABEL maintainer="Kit Ewbank <Kit_Ewbank@hotmail.com>"

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUST_VERSION=1.41.1

RUN yum install -y gcc gcc-c++ openssl-devel;\
    curl https://sh.rustup.rs -sSf | sh -s -- --no-modify-path --profile minimal --default-toolchain $RUST_VERSION -y; \
    chmod -R a+w $RUSTUP_HOME $CARGO_HOME; \
    rustup --version; \
    cargo --version; \
    rustc --version;

WORKDIR /volume
