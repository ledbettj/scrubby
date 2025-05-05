FROM rust:1.86-slim-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN apt-get update && apt-get install build-essential autoconf automake autotools-dev libopus0 libopus-dev pkg-config libclang-dev cmake --yes
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin scrubby2

# We do not need the Rust toolchain to run the binary!
FROM debian:bookworm-slim AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/scrubby2 /app/scrubby2
RUN apt-get update && apt-get install libopus0
#COPY --from=builder /app/plugins /app/plugins

ENTRYPOINT ["/app/scrubby2"]
