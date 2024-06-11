# Stage 1 build
FROM rust:1.78 as builder

WORKDIR /app

COPY . ./

RUN cargo build --release

# Stage 2 build
FROM debian:stable

WORKDIR /app

RUN apt update -y && apt install openssl -y

COPY --from=builder /app/target/release/guardian .
COPY ./certs ./certs
COPY ./config ./config
COPY ./templates ./templates

EXPOSE 8080

CMD ["./guardian"]
