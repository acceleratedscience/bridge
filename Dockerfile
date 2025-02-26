# Stage 1 build
FROM rust:1.85.0 AS builder

WORKDIR /app

COPY . ./

ARG NOTEBOOK=false
ARG LIFECYCLE=false

RUN if [ "$NOTEBOOK" = "true" ] && [ "$LIFECYCLE" = "false" ]; then \
        echo "Building with Notebook Feature..." \
        && cargo build --release --features notebook; \
	elif [ "$NOTEBOOK" = "true" ] && [ "$LIFECYCLE" = "true" ]; then \
		echo "Building Notebook and Lifecycle Feature..." \
		&& cargo build --release --features notebook,lifecycle; \
    else \
        echo "Building without Notebook Feature..." \
        && cargo build --release; \
    fi

# Stage 2 build
FROM debian:stable-slim

WORKDIR /app

RUN apt update -y && apt install openssl -y && apt install ca-certificates

COPY --from=builder /app/target/release/openbridge .
COPY ./certs ./certs
COPY ./config ./config
COPY ./templates ./templates
COPY ./static ./static

RUN chgrp -R 0 /app && \
	chmod -R g=u /app
USER 1001

EXPOSE 8080

CMD ["./openbridge"]
