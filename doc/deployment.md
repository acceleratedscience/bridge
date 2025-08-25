[&#8592; Back](../#bridge)

# Deployment

### Initiating Bridge locally

1.  Clone the repository
2.  Create the self-signed certificate and asymmetric key pairs

    ```shell
    just certs
    just gen_curve
    ```
- If you intend to public face Bridge, properly obtain certificates from a trusted authority

3.  Copy or rename the provided configuration files:

    -   `config/configurations_sample.toml` &#8594; `config/configurations.toml`
    -   `config/database_sample.toml` &#8594; `config/database.toml`

### Running Bridge locally

1.  Start a local DB instances

    Ensure you have Podman installed on your local machine. If you prefer Docker, updated "podman" commands to "docker" commands

    ```shell
    just local-mongo
    ```

    Optionally you can start a cache instance

    ```shell
    just local-keydb
    ```

2.  Start the Bridge server

    ```shell
    cargo run --feature=full --release
    ```

    The release flag will enable all optimizations and compilation will take a longer time
    
    Look in the Cargo.toml for the available feature flags

3.  See the result at [localhost:8080](https://localhost:8080)

### Destroying Bridge running locally

1.  Stop the Bridge server

    Press `Ctrl + C` in the terminal where the server is running or send a sigterm.

2.  Stop the local DB instances

    ```shell
    just down-local-mongo
    just down-local-keydb
    ```

3.  Clear build artifacts (optional)
    ```shell
    cargo clean
    ```

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
    - Ensure you give it the proper permission to access various namespaces and create CRDs
    - The following was generated with Helm
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
