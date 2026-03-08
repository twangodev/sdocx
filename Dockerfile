FROM rust:1-slim AS builder
WORKDIR /build
COPY . .
RUN cargo build --release --bin sdocx-cli

FROM debian:bookworm-slim
COPY --from=builder /build/target/release/sdocx-cli /usr/local/bin/sdocx-cli
ENTRYPOINT ["sdocx-cli"]