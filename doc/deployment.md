[&#8592; Back](../#guardian)

# Deployment

### Initiating Guardian locally

1.  Clone the repository
2.  Create the self-signed certificate and asymmetric key pairs

        just certs
        just gen-curve

3.  Copy or rename the provided configuration files:

    -   `config/configurations_sample.toml` &#8594; `config/configurations.toml`
    -   `config/database_sample.toml` &#8594; `config/database.toml`

### Running Guardian locally

1.  Start a local MongoDB instance

    Ensure you have Docker or Podman installed on your local machine.

        just local-mongo

1.  Start the Guardian server

        cargo run

### Destroying Guardian running locally

1.  Stop the Guardian server

    Press `Ctrl + C` in the terminal where the server is running or send a sigterm.

2.  Stop the local MongoDB instance

        docker stop mongodb

3.  Clear build artifacts (optional)

        cargo clean

### Updating Guardian on OpenShift

> [!WARNING]
> This will definitely change as processes are automated further in the very near future.
> This section also requires Guardian to already be deployed on OpenShift. Deployment process is still being worked on and streamlined.

1.  Build the Guardian container image

        just build

2.  Get and use login password for ECR

        aws ecr get-login-password --region us-east-1 | docker login --username AWS --password-stdin 260533665315.dkr.ecr.us-east-1.amazonaws.com

3.  Tag the container image

        docker tag guardian:v#.#.# 260533665315.dkr.ecr.us-east-1.amazonaws.com/guardian:v#.#.#

4.  Push the container image to the ECR

        docker push 260533665315.dkr.ecr.us-east-1.amazonaws.com/guardian:v#.#.#

5.  Rotate the ECR secret in OpenShift

        kubectl delete secret -n guardian ecr-registry
        aws ecr get-login-password --region us-east-1 | kubectl create secret docker-registry ecr-registry --docker-server=260533665315.dkr.ecr.us-east-1.amazonaws.com/guardian --docker-username=AWS --docker-password=$(aws ecr get-login-password --region us-east-1)

6.  Delete currently running pod to have OpenShift spin up a new pod using the new image pushed to ECR

        # This is not the "recommended" way of deploying
        # This is a temporary solution while we are actively developing the stage env
        kubectl delete pod -n guardian guardian-tls-<pod_id>

7.  Check and ensure the new pod is running

        kubectl get pods -n guardian
