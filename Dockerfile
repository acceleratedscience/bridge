# Stage 1 build
FROM rust:1.80.1 AS builder

WORKDIR /app

COPY . ./

RUN cargo build --release

# Stage 2 build
FROM debian:stable-slim

WORKDIR /app

RUN apt update -y && apt install openssl -y && apt install ca-certificates

COPY --from=builder /app/target/release/guardian .
COPY ./certs ./certs
COPY ./config ./config
COPY ./templates ./templates
COPY ./static ./static

RUN chgrp -R 0 /app && \
	chmod -R g=u /app
USER 1001

EXPOSE 8080

CMD ["./guardian"]
