# Stage 1 build
FROM rust:1.83.0 AS builder

WORKDIR /app

COPY . ./

ARG NOTEBOOK=false

RUN if [ "$NOTEBOOK" = "true" ]; then \
        echo "Building with Notebook Feature..." \
        && cargo build --release --features notebook; \
	elif [ "$NOTEBOOK" = "true" ] && [ "$LIFECYCLE" = "false" ]; then \
		echo "Building without Notebook Feature..." \
		&& cargo build --release --features notebook,lifecycle; \
    else \
        echo "Building without Notebook Feature..." \
        && cargo build --release; \
    fi

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
