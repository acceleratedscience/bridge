# Default values for build arguments (can be overridden by Dockerfile if not provided here)
NOTEBOOK_DEFAULT := "false"
LIFECYCLE_DEFAULT := "false"
OBSERVE_DEFAULT := "false"
MCP_DEFAULT := "false"
OWUI_DEFFAULT := "false"

# Consolidated build recipe that accepts a comma-separated string of features
# Usage examples:
#   just build_features "notebook,lifecycle"
#   just build_features "mcp,observe"
#   just build_features "notebook"
#   just build_features ""  # (or just `just build_features`) -> uses all defaults
build-features features_string="":
    #!/usr/bin/env bash
    set -e -u -o pipefail

    # Initialize all build args to their default values
    # These will be overridden if the feature is present in features_string
    current_notebook={{NOTEBOOK_DEFAULT}}
    current_lifecycle={{LIFECYCLE_DEFAULT}}
    current_observe={{OBSERVE_DEFAULT}}
    current_mcp={{MCP_DEFAULT}}
    current_owui={{OWUI_DEFFAULT}}

    # If features_string is not empty, parse it
    if [[ -n "{{features_string}}" ]]; then
        # Convert comma-separated string to an array (Bash specific)
        IFS=',' read -r -a features_array <<< "{{features_string}}"

        for feature in "${features_array[@]}"; do
            # Trim whitespace (optional, but good for robustness)
            trimmed_feature=$(echo "$feature" | xargs)
            echo "Processing feature: $trimmed_feature" # Debugging
            if [[ "$trimmed_feature" == "notebook" ]]; then
                current_notebook="true"
            elif [[ "$trimmed_feature" == "lifecycle" ]]; then
                current_lifecycle="true"
            elif [[ "$trimmed_feature" == "observe" ]]; then
                current_observe="true"
            elif [[ "$trimmed_feature" == "mcp" ]]; then
                current_mcp="true"
            elif [[ "$trimmed_feature" == "openwebui" ]]; then
                current_owui="true"
            elif [[ -n "$trimmed_feature" ]]; then # Check if trimmed_feature is not empty
                echo "Warning: Unknown feature '$trimmed_feature' in '{{features_string}}'"
            fi
        done
    fi

    cmd="podman build -t bridge"
    cmd="$cmd --build-arg NOTEBOOK=${current_notebook}"
    cmd="$cmd --build-arg LIFECYCLE=${current_lifecycle}"
    cmd="$cmd --build-arg OBSERVE=${current_observe}"
    cmd="$cmd --build-arg MCP=${current_mcp}"
    cmd="$cmd --build-arg OWUI=${current_owui}"
    cmd="$cmd ."

    echo "Executing: $cmd"
    eval "$cmd"

# --- Build Recipes ---
# build: calls build-features with its default empty features_string, resulting in all default build args
build-notebook: (build-features "notebook")

build-notebook-lifecycle: (build-features "notebook,lifecycle")

build-notebook-lifecycle-observe: (build-features "notebook,lifecycle,observe")

build-notebook-lifecycle-mcp: (build-features "notebook,lifecycle,mcp")

build-full: (build-features "notebook,lifecycle,observe,mcp,openwebui")

# --- Frontend & Minification ---
mini-js:
	uglifyjs ./static/js/main.js -o ./static/js/main.js -c -m

build-front:
	tailwindcss -i ./static/css/input.css -o ./static/css/output.css --minify
	tsc
	uglifyjs ./static/js/main.js -o ./static/js/main.js -c -m

# --- Local Development Services ---
local-mongo:
	podman run -d --rm --name mongodb \
	-e MONGODB_ROOT_PASSWORD="admin123456789" \
	-e MONGODB_USERNAME="bridge-user" -e MONGODB_PASSWORD="admin123456789" -e MONGODB_DATABASE="bridge" \
	-p 27017:27017 bitnami/mongodb:latest

local-mongo-arm:
	podman run -d --rm --name mongodb \
	-e MONGO_INITDB_ROOT_USERNAME="guardian-user" \
	-e MONGO_INITDB_ROOT_PASSWORD="admin123456789" \
	-e MONGO_INITDB_DATABASE="bridge" \
	-p 27017:27017 mongodb/mongodb-community-server

local-keydb:
	podman run -d --rm --name keydb \
	-e KEYDB_PASSWORD="admin123456789" \
	-p 6379:6379 bitnami/keydb:latest

down-local-mongo:
	podman stop mongodb

down-local-keydb:
	podman stop keydb

# --- Watchers ---
watch-tailwind:
	tailwindcss -i ./static/css/input.css -o ./static/css/output.css --minify --watch

watch-rust:
	bacon run-long --features "notebook lifecycle"

watch-backend:
	bacon . --features "notebook lifecycle"

watch:
	bacon --features "notebook lifecycle"

# --- Certificates ---
certs:
	mkdir certs
	@openssl req -x509 -newkey rsa:2048 -nodes -keyout certs/key.pem -out certs/cert.pem -days 365 -subj '/CN=open.accelerator.cafe'

gen-curve:
	@openssl ecparam -name prime256v1 -genkey -noout -out certs/private.ec.key
	@openssl ec -in certs/private.ec.key -pubout -out certs/public-key.pem
