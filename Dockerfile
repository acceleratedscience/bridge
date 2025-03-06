# Stage 1 build
FROM rust:1.85.0 AS builder

WORKDIR /app

COPY . ./

ARG NOTEBOOK=false
ARG LIFECYCLE=false

RUN <<EOF
#!/bin/bash
flags=()
if [ "$NOTEBOOK" = "true" ]; then
	flags+=("notebook")
fi
if [ "$LIFECYCLE" = "true" ]; then
	flags+=("lifecycle")
fi
if [ ${#flags[@]} -eq 0 ]; then
	cargo build --release
else
	cargo build --release --features "${flags[@]}"
fi
EOF

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
