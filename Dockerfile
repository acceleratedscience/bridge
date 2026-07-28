# Stage 1 build
FROM rust:1.97.1 AS builder

WORKDIR /app

COPY . ./

ARG NOTEBOOK=false
ARG LIFECYCLE=false
ARG OBSERVE=false
ARG MCP=false
ARG OWUI=false
ARG PPV2=false

RUN <<EOF
#!/bin/bash

# install dx using cargo
cargo install dioxus-cli --locked
dx build -p frontend --release

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
if [ "$OWUI" = "true" ]; then
	flags+=("openwebui")
fi
if [ "$PPV2" = "true" ]; then
	flags+=("ppv2")
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

COPY --from=builder /app/target/release/bridge .
COPY ./certs ./certs
COPY ./config ./config
COPY ./templates ./templates
COPY ./static ./static
COPY ./frontend ./frontend
COPY --from=builder /app/target/dx/frontend/release/web/public ./frontend/public

RUN chgrp -R 0 /app && \
	chmod -R g=u /app
USER 1001

EXPOSE 8080
EXPOSE 8000 

CMD ["./bridge"]
