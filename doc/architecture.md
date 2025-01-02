[&#8592; Back](../#guardian)

# Architecture

> [!NOTE]
> This project is under active development. The documentations may change as the project evolves.

<br>

### Technology powering Guardian

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
	elb["Elasitc Load Balancer"]
	guardian["Guardian"]
	model1["Model Service 1"]
	model2["Model Service 2"]
	model3["Model Service 3"]
	mongodb[("MongoDB")]
	cache[("In-memory Cache")]

	user --> route
	subgraph "AWS"
	route --> elb --> guardian
	subgraph "Openshift"
	guardian --> mongodb
    guardian -- optional --> cache
	guardian --> model1
	guardian --> model2
	guardian --> model3
	end
	end
```

<br>

### Possible Future Architecture

```mermaid
flowchart TD
	user["User"]
	route["Route 53"]
	elb["Elasitc Load Balancer"]
	waf["Web Application Firewall"]
	redis[("Redis")]
	guardian["Guardian"]
	model1["Model Service 1"]
	model2["Model Service 2"]
	model3["Model Service 3"]
	mongodb[("MongoDB")]

	user --> route
	subgraph "AWS"
	route --> waf -- Security  Group --> elb -- Security Group --> guardian
	subgraph "Openshift"
	guardian -- Security Group --> mongodb
	guardian -- Security Group --> redis
	guardian --> model1
	guardian --> model2
	guardian --> model3
	end
	end
```
