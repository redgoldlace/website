# syntax=docker/dockerfile:1

FROM lukemathwalker/cargo-chef:latest-rust-latest AS rust_chef
WORKDIR /website

FROM rust_chef AS rust_planner
WORKDIR /website
COPY --from=website . .
RUN cargo chef prepare --recipe-path recipe.json

FROM rust_chef AS rust_builder
WORKDIR /website
COPY --from=rust_planner /website/recipe.json .
RUN cargo chef cook --release --recipe-path recipe.json
COPY --from=website . .
RUN cargo build --release

FROM node:18 AS js_builder
WORKDIR /website
COPY --from=decktracker . .
RUN yarn install
RUN yarn build

FROM debian:bullseye-slim AS runtime
WORKDIR /website
COPY --from=rust_builder /website/target/release/website /usr/local/bin/kaylynn.gay
COPY --from=js_builder /website/dist /usr/local/share/kaylynn.gay/static/decktracker
COPY --from=website_content . /usr/local/share/kaylynn.gay/

WORKDIR /usr/local/share/kaylynn.gay
ENV ROCKET_PROFILE=release
ENV ROCKET_ADDRESS=0.0.0.0
ENV ROCKET_PORT=8080
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/kaylynn.gay"]