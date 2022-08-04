FROM lukemathwalker/cargo-chef:latest-rust-latest AS chef
WORKDIR /website

FROM chef AS planner
WORKDIR /website
COPY website/ .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
WORKDIR /website
COPY --from=planner /website/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY website/ .
RUN cargo build --release

FROM node:18 AS js_builder
WORKDIR /website
COPY decktracker/ .
RUN yarn install

FROM debian:bullseye-slim AS runtime
WORKDIR /website
COPY --from=builder /website/target/release/website /usr/local/bin/kaylynn.gay
COPY --from=js_builder /website/dist /usr/local/share/kaylynn.gay/static/decktracker
COPY content/ /usr/local/share/kaylynn.gay/

WORKDIR /usr/local/share/kaylynn.gay
ENTRYPOINT ["/usr/local/bin/kaylynn.gay"]