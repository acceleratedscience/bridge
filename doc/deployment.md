[&#8592; Back](../#bridge)

# Deployment

<br>

### Initiating OpenBridge locally

1.  Clone the repository
2.  Create the self-signed certificate and asymmetric key pairs

        just certs
        just gen-curve

3.  Copy or rename the provided configuration files:

    -   `config/configurations_sample.toml` &#8594; `config/configurations.toml`
    -   `config/database_sample.toml` &#8594; `config/database.toml`

<br>

### Running OpenBridge locally

1.  Start a local MongoDB instance

    Ensure you have Podman installed on your local machine. If you prefer Docker, updated "podman" commands to "docker" commands

        just local-mongo

1.  Start the OpenBridge server

        cargo run

1.  See the result at [localhost:8080](https://localhost:8080)

<br>

### Destroying OpenBridge running locally

1.  Stop the OpenBridge server

    Press `Ctrl + C` in the terminal where the server is running or send a sigterm.

1.  Stop the local MongoDB instance

        podman stop mongodb

1.  Clear build artifacts (optional)

        cargo clean

<br>

### Updating OpenBridge on OpenShift

> [!WARNING]
> This will definitely change as processes are automated further in the very near future.
> This section also requires OpenBridge to already be deployed on OpenShift. Deployment process is still being worked on and streamlined.

1.  Build the OpenBridge container image

        just build

1.  Get and use login password for ECR

        aws ecr get-login-password --region us-east-1 | podman login --username AWS --password-stdin 260533665315.dkr.ecr.us-east-1.amazonaws.com

1.  Tag the container image

        podman tag bridge:v#.#.# 260533665315.dkr.ecr.us-east-1.amazonaws.com/bridge:v#.#.#

1.  Push the container image to the ECR

        podman push 260533665315.dkr.ecr.us-east-1.amazonaws.com/bridge:v#.#.#

1.  Rotate the ECR secret in OpenShift

        kubectl delete secret -n bridge ecr-registry
        aws ecr get-login-password --region us-east-1 | kubectl create secret docker-registry ecr-registry --docker-server=260533665315.dkr.ecr.us-east-1.amazonaws.com/bridge --docker-username=AWS --docker-password=$(aws ecr get-login-password --region us-east-1)

1.  Delete currently running pod to have OpenShift spin up a new pod using the new image pushed to ECR

        # This is not the "recommended" way of deploying
        # This is a temporary solution while we are actively developing the stage env
        kubectl delete pod -n bridge bridge-tls-<pod_id>

1.  Check and ensure the new pod is running

        kubectl get pods -n bridge
