FROM rust:1-trixie AS builder

WORKDIR /app
COPY . .
RUN cargo install --path .

FROM gcr.io/distroless/cc-debian13:latest
LABEL maintainer="Jetsung Chan<i@jetsung.com"

WORKDIR /app

COPY --from=builder /usr/local/cargo/bin/filetas /usr/local/bin/filetas
COPY --from=builder /app/templates /app/templates

EXPOSE 3000

ENTRYPOINT ["filetas"]
