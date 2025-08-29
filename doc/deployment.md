[&#8592; Back](../#bridge)

# Deployment

### Initiating Bridge locally

1.  Clone the repository
2.  Create the self-signed certificate and asymmetric key pairs

    ```shell
    just certs
    just gen-curve
    ```

    > **Note:** If you intend to deploy Bridge publicly, properly obtain certificates from a trusted authority.

3.  Copy or rename the provided configuration files:

    -   `config/configurations_sample.toml` &#8594; `config/configurations.toml`
    -   `config/database_sample.toml` &#8594; `config/database.toml`

4.  Update the relevant variables:

    **configurations.toml**

    -   `redirect_url`: Set to their localhost versions (commented out)
    -   `client_id` / `client_secret`: If you're with IBM, you can find instructions on how to generate these [here](https://github.com/acceleratedscience/configurations/tree/main/bridge/prod).  
        If you're developing your own application, you will need to register it with IBM ID to use the IBM ID authentication. For now, no other auth methods are supported (PRs welcome)

    **database.toml**

    -   `[mongodb]`: Use the urls without auth (commented out) if you're using an Apple Silicon device (M1/M2/M3 etc.)

<br>

### Running Bridge locally

1.  Start a local DB instance

    Ensure you have Podman installed and running on your local machine.

    ```shell
    just local-mongo
    ```

    > **Apple Silicon Support:** Use the ARM install instead, and set the DB urls in `database.toml` without auth: `url="mongodb://127.0.0.1:27017/bridge"`
    >
    > ```
    > just local-mongo-arm
    > ```

    > **Docker Support:** If you prefer Docker, updated "podman" commands to "docker" commands in the [justfile](../justfile).

2.  Optionally you can start a cache instance

    ```shell
    just local-keydb
    ```

3.  Start the Bridge server

    ```shell
    cargo run --features=full --release
    ```

    The `--release` flag will enable all optimizations and compilation will take a longer time.  
    Refer to [Cargo.toml](../Cargo.toml) for the available feature flags

    > **Development:**
    > To have the server restart on change, use cargo-watch:
    >
    > ```
    > cargo install cargo-watch
    > ```
    >
    > ```
    > cargo watch -x 'run --features=full --release'
    > ```
    >
    > **Note:** Be patient as the initial build may take multiple minutes.

4.  See the result at [localhost:8080](https://localhost:8080) (HTTPS required)

<br>

### Destroying Bridge running locally

1.  Stop the Bridge server

    Simply press `Ctrl` + `C` in the terminal where the server is running or send a sigterm.

2.  Stop the local DB instances

    ```shell
    just down-local-mongo
    just down-local-keydb
    ```

3.  Clear build artifacts (optional)
    ```shell
    cargo clean
    ```

<br>

### Deployment to Kubernetes / Openshift

> [!NOTE]
> This is one possible way to deploy and it is not a hard requirement.

1.  Build the Bridge container image

    Check what features you want to enable for your deployment

    ```shell
    just build-full
    ```

2.  Tag and push the image to your choice of Image repository

3.  Apply this service as a deployment

    -   Ensure you give it the proper permission to access various namespaces and create CRDs
    -   The following was generated with Helm:

        ```yaml
        kind: Deployment
        metadata:
            annotations:
                deployment.kubernetes.io/revision: "39"
                meta.helm.sh/release-name: bridge-openad
                meta.helm.sh/release-namespace: openbridge
            creationTimestamp: "2025-05-15T03:42:42Z"
            generation: 39
            labels:
                app.kubernetes.io/instance: bridge-openad
                app.kubernetes.io/managed-by: Helm
                app.kubernetes.io/name: bridge-openad
                app.kubernetes.io/version: 1.16.0
                helm.sh/chart: bridge-openad-0.1.0
            name: bridge-openad
            namespace: bridge
            resourceVersion: ""
            uid: ""
        spec:
            progressDeadlineSeconds: 600
            replicas: 1
            revisionHistoryLimit: 10
            selector:
                matchLabels:
                    app.kubernetes.io/instance: bridge-openad
                    app.kubernetes.io/name: bridge-openad
            strategy:
                rollingUpdate:
                    maxSurge: 25%
                    maxUnavailable: 25%
                type: RollingUpdate
            template:
                metadata:
                    annotations:
                        kubectl.kubernetes.io/restartedAt: "2025-07-04T22:24:19-04:00"
                    creationTimestamp: null
                    labels:
                        app.kubernetes.io/instance: bridge-openad
                        app.kubernetes.io/managed-by: Helm
                        app.kubernetes.io/name: bridge-openad
                        app.kubernetes.io/version: 1.16.0
                        helm.sh/chart: bridge-openad-0.1.0
                spec:
                    containers:
                        - image: xxx.amazonaws.com/bridge/openad:v0.0.1
                        imagePullPolicy: Always
                        livenessProbe:
                            failureThreshold: 3
                            httpGet:
                                path: /health
                                port: 8080
                                scheme: HTTPS
                            periodSeconds: 10
                            successThreshold: 1
                            timeoutSeconds: 1
                        name: bridge-openad
                        ports:
                            - containerPort: 8080
                                name: http
                                protocol: TCP
                        readinessProbe:
                            failureThreshold: 3
                            httpGet:
                                path: /health
                                port: 8080
                                scheme: HTTPS
                            periodSeconds: 10
                            successThreshold: 1
                            timeoutSeconds: 1
                        resources:
                            requests:
                                cpu: 100m
                                memory: 1Gi
                        terminationMessagePath: /dev/termination-log
                        terminationMessagePolicy: File
                    dnsPolicy: ClusterFirst
                    imagePullSecrets:
                        - name: ecr-registry-openad
                    restartPolicy: Always
                    schedulerName: default-scheduler
                    securityContext: {}
                    serviceAccount: bridge-openad
                    serviceAccountName: bridge-openad
                    terminationGracePeriodSeconds: 30
        ```
