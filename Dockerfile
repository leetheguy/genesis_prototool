# genesis-serve, with the browser bundle it hands out. See docker-compose.yml and docs/setup.md.

FROM rust:1-bookworm AS build
RUN rustup target add wasm32-unknown-unknown
# trunk (bundles the wasm) and Tailwind's standalone binary, at the versions docs/setup.md pins.
RUN curl -sSfL https://github.com/trunk-rs/trunk/releases/download/v0.22.2/trunk-x86_64-unknown-linux-gnu.tar.gz \
      | tar -xz -C /usr/local/bin \
 && curl -sSfL -o /usr/local/bin/tailwindcss \
      https://github.com/tailwindlabs/tailwindcss/releases/download/v4.3.5/tailwindcss-linux-x64 \
 && chmod +x /usr/local/bin/tailwindcss
WORKDIR /src
COPY . .
RUN cargo build --release -p genesis-serve \
 && cd apps/web && trunk build --release

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/genesis-serve /usr/local/bin/genesis-serve
COPY --from=build /src/apps/web/dist /app/dist
WORKDIR /app
EXPOSE 8080
HEALTHCHECK --interval=10s --timeout=3s --start-period=5s CMD curl -fsS http://127.0.0.1:8080/health || exit 1
CMD ["genesis-serve", "--configs", "/app/configs", "--state", "/app/state", "--dist", "/app/dist", "--listen", "0.0.0.0:8080"]
