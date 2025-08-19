[&#8592; Back](../#bridge)

# Updating Bridge on OpenShift or K8s
> [!WARNING]
> This will definitely change as processes are automated further in the very near future.
> This section also requires Bridge to already be deployed on OpenShift. Deployment process is still being worked on and streamlined.

1.  Build the Bridge container image
    ```shell
    just build-full
    ```

1.  Get and use login password for ECR
    ```shell
    aws ecr get-login-password --region us-east-1 | podman login --username AWS --password-stdin xxx.amazonaws.com
    ```

1.  Tag the container image
    ```shell
    podman tag bridge:v#.#.# xxx.amazonaws.com/bridge:v#.#.#
    ```

1.  Push the container image to the ECR
    ```shell
    podman push xxx.amazonaws.com/bridge:v#.#.#
    ```

1.  Rotate the ECR secret in OpenShift
    ```shell
    kubectl delete secret -n bridge ecr-registry
    aws ecr get-login-password --region us-east-1 | kubectl create secret docker-registry ecr-registry --docker-server=xxx.amazonaws.com/bridge --docker-username=AWS --docker-password=$(aws ecr get-login-password --region us-east-1)
    ```

1.  Delete currently running pod to have OpenShift spin up a new pod using the new image pushed to ECR
    ```shell
    # This is not the "recommended" way of deploying new version of Bridge on your infrastructure
    # This is a temporary solution while we are actively developing an automated CICD process
    kubectl delete pod -n bridge bridge-tls-<pod_id>
    ```

1.  Check and ensure the new pod is running
    ```shell
    kubectl get pods -n bridge
    ```

# Runbook

> [!NOTE]
> This project is under active development. The run book may change as the project evolves. The run book may also not cover everything. Reach out to the maintainers for the most up-to-date information.

> What could go wrong as of (2025-08-15)?

<br>

It's best to first think about how the data flows through the Bridge:

```mermaid
flowchart LR
	ingress["Ingress from User"]
	bridge["Bridge"]
	model["Model Services"]
	mongodb[("MongoDB")]
    redis[("Cache")]

	ingress -- Let's Encrypt --> bridge
	bridge --> mongodb
    bridge --> redis
	bridge --> model
```

WhatN does the authenication flow look like?

```mermaid
sequenceDiagram
	autonumber
	User ->>+ Bridge: Login
	Bridge -->> Bridge: Redirect to IBM Verify
	User -->> IBM Verify: I would like to login
	IBM Verify -->> IBM Verify: Verify User or Register User
	IBM Verify ->> Bridge: User Profile
	Note over Bridge: If new user<br>Add user to Database
	Bridge ->> Bridge: Redirect to User Portal w/ cookie
	User ->> Bridge: User Portal
	Bridge ->> User: User Portal
```

What does the user initiated token-based (i.e., inferences) flow look like?

```mermaid
sequenceDiagram
	autonumber
	User ->>+ Bridge: I would like an access token (through user portal)
	Bridge -->> Database: What models does this user have access to?
	Database -->> Bridge: User has access to Model Service A
	Bridge ->>- User: Here is your token
	Note over User: You have access to<br>Model Service A<br>Your token will reflect that

	autonumber 1
	User ->>+ Bridge: I would like to access Model Service B
	Bridge ->> Bridge: Inspect token for Model Service B
	Bridge -x- User: You do not have access to Model Service B

	box Black Not Internet Facing
	participant Database
	participant Model Service A
	end

	autonumber 1
	User ->>+ Bridge: I would like to access Model Service A
	Bridge ->> Bridge: Inspect token for Model Service A
	Bridge -->> Model Service A: Request from User
	Model Service A -->> Bridge: Response to User
	Bridge ->>- User: Response from Model Service A
```

What does the user initiated cookie-based (i.e., web applications) flow look like?

```mermaid
sequenceDiagram
	autonumber
	User ->>+ Bridge: I would like to log in
    create participant IBMID
    Bridge ->> IBMID: Redirect user to IBMID
    IBMID ->> Bridge: User authenticated
    Bridge ->> User: You've been authenticated
    Note over User: Cookie is sent back <br>with access information
    User -->> Bridge: I would like to access Web App A
    Note over Bridge: User does have access to Web App A

	box Black Not Internet Facing
	participant Web App A
	end

    Bridge -->> Web App A: Request proxied for the User
    Web App A -->> Bridge: Response from Request
	Bridge ->>- User: Response proxied to User
```

> [!TIP]
> Any of the arrows in the diagrams above could error. When encountering a problem with Bridge, it is best to determine where the problem occurred.
>
> Possible failures:
>
> -   If the user is unable to login, the problem could be with IBM ID.
> -   If the user is unable to generate access tokens, the problem could be with the database. It could also be Bridge specific.
> -   If the user is unable to access a model service, the problem could be with the model service.
>     -   Find out what status could was returned to the user
>     -   The model service could be down. Check the health of the model service either through kubectl / oc or the OpenShift console

> I have narrowed down to where the problem may be originating from. What do I do next?

The best thing to do next is to check the logs from Bridge. You can check the logs through the OpenShift console or through the command line. You can enter the following command to check the logs from Bridge through the command line:

```shell
kubectl logs -n bridge bridge-tls-<pod_id>
```

### Example #1

> Problem: User reports that they are unable to login. When they try to access the group-admin page, they were faced with a forbidden code 403 error.

1. We check the log

    ```shell
    kubectl logs -n bridge bridge-tls-<pod_id>
    ```
    Output:

    ```
    2024-08-01T13:42:45.523259Z  WARN ThreadId(34) bridge::web::bridge_middleware::cookie_check: src/web/bridge_middleware/cookie_check.rs:235: Bridge cookie not found from ip Some(100.64.0.4:45712)
    ```

    We see that the Bridge could not find a cookie from the user's browser.

2. Let's take a closer look in the codebase: cookie_check.rs line 235.
    ```rust
    // code from cookie_check.rs as of 2024-08-01
    match req.cookie(COOKIE_NAME).map(|c| c.value().to_string()) {
        Some(v) => {
            let bridge_cookie_result = serde_json::from_str::<BridgeCookie>(&v);
            match bridge_cookie_result {
                Ok(gcs) => {
                    // also insert notebook_status_cookie if available
                    if let Some(ncs) = req.cookie(NOTEBOOK_STATUS_COOKIE_NAME)
                        && let Ok(ncs) =
                            serde_json::from_str::<NotebookStatusCookie>(ncs.value())
                    {
                        req.extensions_mut().insert(ncs);
                    }

                    #[cfg(feature = "openwebui")]
                    {
                        use crate::{auth::OWUI_COOKIE_NAME, db::models::OWUICookie};

                        if let Some(owui_cookie) = req.cookie(OWUI_COOKIE_NAME)
                            && let Ok(oc) =
                                serde_json::from_str::<OWUICookie>(owui_cookie.value())
                            {
                                req.extensions_mut().insert(oc);
                            }
                    }

                    req.extensions_mut().insert(gcs.clone());

                    let service = self.service.clone();
                    async move {
                        // sesssion_id management only enabled if cache is present
                        if let Some(cache) = CACHEDB.get() {
                            if let Some(session_id) = &gcs.session_id {
                                match cache.get_session_id(&gcs.subject).await {
                                    Ok(session_id_retreived) => {
                                        if session_id_retreived.eq(session_id) {
                                            return Ok(service.call(req).await?.map_into_left_body());
                                        }
                                        warn!(
                                            "Session id does not match cache for user: {:?} ip: {:?}",
                                            gcs.subject,
                                            req.connection_info().realip_remote_addr()
                                        );
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Session id not found in cache for user: {:?} ip: {:?} error: {:?}", 
                                            gcs.subject,
                                            req.connection_info().realip_remote_addr(),
                                            e
                                        );
                                    }
                                }
                            } else {
                                warn!(
                                    "Session id not found in bridge cookie for user: {:?} ip: {:?}",
                                    gcs.subject,
                                    req.connection_info().realip_remote_addr()
                                );
                            }
                            return Ok(req.into_response(
                                HttpResponse::Unauthorized().finish().map_into_right_body(),
                            ));
                        }

                        Ok(service.call(req).await?.map_into_left_body())
                    }
                    .boxed_local()
                }
                Err(e) => {
                    warn!("Bridge cookie deserialization error: {:?}", e);
                    let res = HttpResponse::InternalServerError()
                        .finish()
                        .map_into_right_body();
                    Box::pin(async { Ok(req.into_response(res)) })
                }
            }
        }
        None => {
            // Make sure "X-Forwarded-For" is present in the header
            warn!(
                "Bridge cookie not found from ip {:?}", // <-- line 235
                req.connection_info().realip_remote_addr()
            );
            let res = HttpResponse::Unauthorized().finish().map_into_right_body();
            Box::pin(async { Ok(req.into_response(res)) })
        }
    }
    ```
    While some knowledge of the Rust programming language would help quite a bit, we can see that in this pattern match that indeed there is no cookie.

### Example #2

> Problem: User tried to log in using IBM ID with their @ibm email address. They successfully were redirected to IBM Verify, but when they were redirected back to Bridge, they were faced with an internal server error.

1. Check the logs

    ```shell
    kubectl logs -n bridge bridge-tls-<pod_id>
    ```
    Output:
    ```
    2024-08-01T14:45:27.416619Z ERROR ThreadId(28) bridge::web::route::auth: src/web/route/auth/mod.rs:146: Error: Kind: An error occurred when trying to execute a write operation: WriteError(WriteError { code: 11000, code_name: None, message: "E11000 duplicate key error collection: bridge.users index: email dup key: { email: \"choi@ibm.com\" }", details: None }), labels: {}
    ```

    There is a lot of information here, but the key part is the "message" field. It seems that there is a duplicate key error in the database. This could be because the user already exists in the database.

    If you port-forward to the MongoDB pod, you can check the User collection, indeed "choi@ibm.com" exists. Since there is a Unique Index on the email field, you cannot have duplicate emails.

    Why is Bridge trying to register this user if they already exist? Let's look at the code:

    ```rust
    // code from auth/mod.rs as of 2024-08-01
    let (id, user_type) = match r {
    	Ok(user) => (user._id, user.user_type),
    	// user not found, create user
    	Err(_) => {
    		// get current time in time after unix epoch
    		let time = time::OffsetDateTime::now_utc();
    		// add user to the DB as a new user
    		let r = log_error!( // <-- Line 146
    			dt.insert(
    				User {
    					_id: ObjectId::new(),
    					sub: subject,
    					user_name: name.clone(),
    					email: email.clone(),
    					groups: vec![],
    					user_type: UserType::User,
    					created_at: time,
    					updated_at: time,
    					last_updated_by: email,
    				},
    				USER,
    			)
    			.await
    		)?;
    		(
    			r.as_object_id().ok_or_else(|| {
    				BridgeError::GeneralError("Could not convert BSON to objectid".to_string())
    			})?,
    			UserType::User,
    		)
    	}
    };

    ```

    As seen above, we are indeed failing to insert the user due to the unique index. In this pattern match, we see we are matching "r" with Err. A little above, we see the following code:

    ```rust
    let r: Result<User> = db
    	.find(
    		doc! {
    			"sub": &subject
    		},
    		USER,
    	)
    	.await;
    ```

    Looks like when the user is redirected back to Bridge from IBM Verify with the user profile data, we are querying out databsed based on the "sub" field. That's weird, why "sub" and not "email?" This is because the "sub" field is guaranteed by IBM Verify to be unique and specifically tied to a specific user.

    No super clear due to the technical details to our upstream authorization service, but our user originally authenticated with W3 and then later tried to log in with IBM ID. The User needs to use W3 to log in.

    Once the maintainers implement a check to that the email has been verified, we can update the code above to query based on the email field and send back the proper error message.
