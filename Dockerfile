# Stage 1 build
FROM rust:1.88.0 AS builder

WORKDIR /app

COPY . ./

ARG NOTEBOOK=false
ARG LIFECYCLE=false
ARG OBSERVE=false
ARG MCP=false

RUN <<EOF
#!/bin/bash
flags=()
if [ "$NOTEBOOK" = "true" ]; then
	flags+=("notebook")
fi
if [ "$LIFECYCLE" = "true" ]; then
	flags+=("lifecycle")
fi
if [ "$OBSERVE" = "true" ]; then
	flags+=("observe")
fi
if [ "$MCP" = "true" ]; then
	flags+=("mcp")
fi
if [ ${#flags[@]} -eq 0 ]; then
	echo "Building with no features..."
	cargo build --release
else
	features_string=$(IFS=,; echo "${flags[*]}")
    echo "Building with features: $features_string"
    cargo build --release --features "$features_string"
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
EXPOSE 8000 

CMD ["./openbridge"]
