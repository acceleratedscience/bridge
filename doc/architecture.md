[&#8592; Back](../#OpenBridge)

# Architecture

> [!NOTE]
> This project is under active development. The documentations may change as the project evolves.

<br>

### Technology powering OpenBridge

-   Front-end:
    -   HTMX
    -   TypeScript
    -   Tailwind CSS
-   Back-end:
    -   Rust
-   Infrastructure:
    -   OpenShift / Kubernetes
    -   Podman
        -   Primarily used for containerizing the application
    -   Helm
    -   OpenTofu
-   Database:
    -   MongoDB
    -   Redis / KeyDB (Optional)
-   Dev Tooling:
    -   rust-analyzer
    -   bacon
    -   tsc
    -   typescript-language-serve (ts_ls)
    -   tailwind cli

<br>

### Current Architecture

```mermaid
flowchart LR
	user["User"]
	route["Route 53"]
	elb["Network Load Balancer"]
	ob["Bridge"]
	model1["Model Service 1"]
	model2["Model Service 2"]
	model3["Model Service 3"]
	mongodb[("MongoDB")]
	cache[("In-memory Cache")]

	user --> route
	subgraph "AWS"
	route --> elb --> ob
	subgraph "Openshift"
	ob --> mongodb
    ob -- optional --> cache
	ob --> model1
	ob --> model2
	ob --> model3
	end
	end
```

<br>

### Possible Future Architecture

```mermaid
flowchart TD
	user["User"]
	route["Route 53"]
	ppp["Proxy Protocol V2"]
	elb["Network Load Balancer"]
	redis[("Redis")]
	ob["Bridge"]
	model1["Model Service 1"]
	model2["Model Service 2"]
	model3["Model Service 3"]
	mongodb[("MongoDB")]

	user --> route
	subgraph "AWS"
	route --> elb -- Security Group --> ppp --> ob
	subgraph "Openshift"
	ob -- Security Group --> mongodb
	ob -- Security Group --> redis
	ob --> model1
	ob --> model2
	ob --> model3
	end
	end
```
