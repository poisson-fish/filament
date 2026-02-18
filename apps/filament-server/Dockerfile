FROM rust:1.93.1-slim-trixie AS builder
WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps ./apps

RUN cargo build --release -p filament-server

FROM debian:trixie-slim
RUN apt-get update \
	&& apt-get install -y --no-install-recommends ca-certificates \
	&& rm -rf /var/lib/apt/lists/*
RUN useradd --system --home /nonexistent --shell /usr/sbin/nologin filament
WORKDIR /app

COPY --from=builder /build/target/release/filament-server /usr/local/bin/filament-server

RUN mkdir -p /var/lib/filament/attachments && chown -R filament:filament /var/lib/filament

USER filament
EXPOSE 3000
CMD ["/usr/local/bin/filament-server"]
