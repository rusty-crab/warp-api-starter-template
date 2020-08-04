# Stage 1 - build 
FROM rust:1.45.2-slim as build-deps

LABEL maintainer="aslamplr@gmail.com"
LABEL version=1.0

WORKDIR /src-root

RUN apt-get update && apt-get install -y build-essential libssl-dev pkg-config 
# required for auto-reload in development only.
RUN cargo install systemfd cargo-watch
# clang, llvm required for argonautica dependency.
RUN apt-get install -y clang llvm-dev libclang-dev

# install movine for database migrations
RUN apt-get install -y libsqlite3-dev wait-for-it
RUN cargo install movine

ENTRYPOINT ["wait-for-it", "db:5432", "--", "./scripts/run_dev.sh"]

