FROM rust:latest AS builder
WORKDIR /build
ADD . .
RUN cargo build -p pahkat-server --release

FROM debian:stretch-slim
RUN apt-get update && apt-get install -y libsqlite3-dev
WORKDIR /app/
COPY --from=builder /build/target/release/pahkat-server .
ENV ARTIFACTS_DIR artifacts
ENV DATABASE_URL db/db.sqlite
VOLUME artifacts db
CMD ["./pahkat-server", "--bind", "0.0.0.0", "--port", "8080", "--path", "repo"]