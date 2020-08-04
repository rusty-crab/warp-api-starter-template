# Stage 1 - build 
FROM rust:1.45.2-slim as build-deps

WORKDIR /usr/src/web-app

RUN apt-get update && apt-get install -y build-essential libssl-dev pkg-config 
# clang, llvm required for argonautica dependency.
RUN apt-get install -y clang llvm-dev libclang-dev

COPY Cargo.* ./
COPY src/ src/

ARG DATABASE_URL
RUN cargo install --path .

# Stage 2 - deploy 
FROM debian:buster-slim

LABEL maintainer="aslamplr@gmail.com"
LABEL version=1.0

WORKDIR /usr/src/web-app

RUN apt-get update && apt-get install -y libssl-dev ca-certificates

COPY --from=build-deps /usr/local/cargo/bin/warp-api-starter-template /usr/local/bin/warp-api-starter-template

ENV RUST_LOG=info
CMD ["warp-api-starter-template"]

