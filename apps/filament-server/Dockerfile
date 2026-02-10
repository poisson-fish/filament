FROM rust:1.93-slim-bookworm AS builder
WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps ./apps

RUN cargo build --release -p filament-server

FROM debian:bookworm-slim
RUN useradd --system --home /nonexistent --shell /usr/sbin/nologin filament
WORKDIR /app

COPY --from=builder /build/target/release/filament-server /usr/local/bin/filament-server

USER filament
EXPOSE 3000
CMD ["/usr/local/bin/filament-server"]
