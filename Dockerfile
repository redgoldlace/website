FROM lukemathwalker/cargo-chef:latest-rust-latest AS chef
WORKDIR /website

FROM chef AS planner
COPY website/ .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder 
COPY --from=planner /website/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY website/ .
RUN cargo build --release

FROM debian:bullseye-slim AS runtime
COPY --from=builder /website/target/release/website /usr/local/bin/kaylynn.gay
COPY content/ /usr/local/share/kaylynn.gay/

WORKDIR /usr/local/share/kaylynn.gay
ENTRYPOINT ["/usr/local/bin/kaylynn.gay"]