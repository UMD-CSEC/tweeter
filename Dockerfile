FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json

# build dependencies
RUN cargo chef cook --release --recipe-path recipe.json

# build app
COPY . .
RUN cargo build --release --bin tweeter

FROM chef AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/tweeter /usr/local/bin
COPY --from=builder /app/views views
COPY --from=builder /app/assets assets

# EST gang
ENV TZ="America/New_York"

EXPOSE 1447
ENTRYPOINT ["/usr/local/bin/tweeter"]
