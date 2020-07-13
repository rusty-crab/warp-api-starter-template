# Stage 1 - build 
FROM rust:1.44-slim as build-deps

LABEL maintainer="aslamplr@gmail.com"
LABEL version=1.0

WORKDIR /src-root

RUN apt-get update && apt-get install -y build-essential libssl-dev pkg-config 
# required for auto-reload in development only.
RUN cargo install systemfd cargo-watch
# clang, llvm required for argonautica dependency.
RUN apt-get install -y clang llvm-dev libclang-dev

ENTRYPOINT ["./scripts/run_dev.sh"]

