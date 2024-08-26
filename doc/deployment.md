[&#8592; Back!](../#guardian)

# Deployment

### Run Guardian locally:

1.  Clone the repository
2.  Create the self-signed certificate and asymmetric key pairs

        bash
        just certs
        just gen-curve

3.  Create configuration files
    -   Use the provided `config/configurations_sample.toml` as a template
    -   Copy or rename this file to `config/configurations.toml`
    -   Same thing for `config/database_sample.toml` to `config/database.toml`
4.  Start a local MongoDB instance
    -   Ensure you have Docker or Podman installed on your local machine
    ```bash
    just local-mongo
    ```
5.  Start the Guardian server
    ```bash
    cargo run
    ```

##### How to destroy Guardian running locally:

1. Stop the Guardian server
    - Press `Ctrl + C` in the terminal where the server is running or send a sigterm
2. Stop the local MongoDB instance
    ```bash
    docker stop mongodb
    ```
3. (Optional) Clear build artifacts
    ```bash
    cargo clean
    ```

##### How to update the Guardian on OpenShift:

> [!WARNING]
> This will definitely change as processes are automated further in the very near future.
> This section also requires Guardian to already be deployed on OpenShift. Deployment process is still being worked on and streamlined.

1. Build the Guardian container image
    ```bash
    just build
    ```
2. Get and use login password for ECR
    ```bash
    aws ecr get-login-password --region us-east-1 | docker login --username AWS --password-stdin 260533665315.dkr.ecr.us-east-1.amazonaws.com
    ```
3. Tag the container image
    ```bash
    docker tag guardian:v#.#.# 260533665315.dkr.ecr.us-east-1.amazonaws.com/guardian:v#.#.#
    ```
4. Push the container image to the ECR
    ```bash
    docker push 260533665315.dkr.ecr.us-east-1.amazonaws.com/guardian:v#.#.#
    ```
5. Rotate the ECR secret in OpenShift
    ```bash
    kubectl delete secret -n guardian ecr-registry
    aws ecr get-login-password --region us-east-1 | kubectl create secret docker-registry ecr-registry --docker-server=260533665315.dkr.ecr.us-east-1.amazonaws.com/guardian --docker-username=AWS --docker-password=$(aws ecr get-login-password --region us-east-1)
    ```
6. Delete currently running pod to have OpenShift spin up a new pod using the new image pushed to ECR
    ```bash
    # This is not the "recommended" way of deploying
    # This is a temporary solution while we are actively developing the stage env
    kubectl delete pod -n guardian guardian-tls-<pod_id>
    ```
7. Check and ensure the new pod is running
    ```bash
    kubectl get pods -n guardian
    ```
