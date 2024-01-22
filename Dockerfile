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

FROM debian:bookworm-slim AS runtime
WORKDIR /website
COPY --from=rust_builder /website/target/release/website /usr/local/bin/kaylynn.gay
COPY --from=js_builder /website/dist /usr/local/share/kaylynn.gay/static/decktracker
COPY --from=website_content . /usr/local/share/kaylynn.gay/

ENV WOEBLOG_PROFILE=release
ENV WOEBLOG_HOST.ADDRESS=0.0.0.0
ENV WOEBLOG_HOST.PORT=8080
ENV WOEBLOG_CONTENT_DIR=/usr/local/share/kaylynn.gay/
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/kaylynn.gay"]
