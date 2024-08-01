<div align="center">
    <img src="../static/favicon.svg" width="100px">
</div>
</br>

<p align="center">
    <a href="https://open.accelerator.cafe" target="_blank">
        :link: Guardian
    </a>
</p>

---

> [!NOTE]
> This project is under active development. The documentations may change as the project evolves.

##### Technology powering Guardian
- Front-end:
    - HTMX
    - JavaScript
- Back-end:
    - Rust
- Infrastructure:
    - OpenShift / Kubernetes
    - Docker
        - Primarily used for containerizing the application
    - Helm
    - OpenTofu
- Database:
    - MongoDB

##### Current Architecture
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

    user --> route
    subgraph "AWS"
    route --> elb --> guardian
    subgraph "Openshift"
    guardian --> mongodb
    guardian --> model1
    guardian --> model2
    guardian --> model3
    end
    end
```

##### Possible Future Architecture
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
