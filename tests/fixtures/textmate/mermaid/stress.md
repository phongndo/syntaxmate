# Atlas platform diagrams
These hand-maintained views describe the same deployment from complementary angles. Labels intentionally include naïve, 東京, λ, and 🚀.
```mermaid architecture
architecture-beta
  %% Regional topology
  group edge(cloud)[Edge Layer]
  group core(server)[Core Platform]
  service dns(internet)[Global DNS] in edge
  service gateway(server)[API Gateway] in edge
  service ledger(database)[Ledger] in core
  junction ingress in edge
  dns:R --> L:ingress
  ingress{group}:B --> T:gateway
  gateway:R <--> L:ledger
```
~~~mermaid
classDiagram
  %% Domain model with generic and visibility forms
  class Order~T~ {
    +String id
    #List~T~ items
    -Money total
    +addItem(T item) Order~T~
    +checkout(Payment payment)$ Receipt
  }
  class Receipt
  <<service>> CheckoutService
  Order "1" *-- "1..*" LineItem : contains
  CheckoutService ..> Order : validates
  Customer --> Order : places
  Order : +cancel(String reason) bool
  Receipt : String confirmation
~~~
```{.mermaid data-view=storage}
erDiagram
  %% Logical storage model
  CUSTOMER ["Account owner"] {
    uuid id PK "stable identifier"
    string display_name "Unicode résumé"
    string email UK
  }
  ORDER {
    uuid id PK
    uuid customer_id FK
    decimal total
  }
  LINE_ITEM {
    uuid order_id PK, FK
    string sku PK
  }
  CUSTOMER ||--o{ ORDER : "places orders"
  ORDER ||--|{ LINE_ITEM : contains
```

```mermaid
gantt
  title Atlas release train
  dateFormat YYYY-MM-DD
  axisFormat %d/%m
  tickInterval 1week
  excludes weekends, 2026-07-14
  todayMarker stroke-width:2px,stroke:#d33,opacity:0.7
  section Foundation
  Schema review :done, schema, 2026-07-01, 5d
  API migration :crit, active, api, after schema, 8d
  section Delivery
  Canary rollout :canary, after api, 4d
  General availability :milestone, ga, 2026-07-25, 0d
```

```mermaid
gitGraph
  commit id: "bootstrap" tag: "v0.1"
  branch feature-search order: 2
  checkout feature-search
  commit id: "index café" type: HIGHLIGHT
  branch experiment
  commit id: "vector λ" type: REVERSE
  checkout feature-search
  cherry-pick id: "bootstrap"
  checkout main
  merge feature-search tag: "v1.0 🚀"
  commit id: "stabilize" type: NORMAL
```

```mermaid interactive
flowchart TD
  %% Shapes, links, labels, classes, and nested direction
  request[/Incoming request/] --> auth{"Token valid?"}
  auth -- no --> denied>Access denied]
  auth ==>|yes| route@{ shape: decision, label: "Choose region" }
  route --> eu["EU · Paris"]
  route --> apac["APAC · 東京"]
  subgraph compute["Compute fleet"]
    direction LR
    eu o--o scheduler{{Scheduler}}
    apac x--x scheduler
    scheduler --> worker1[[Worker one]]
    scheduler -. telemetry .-> worker2[(Worker two)]
  end
  worker1 & worker2 --> response(((Response)))
  classDef boundary fill:#eef,stroke:#55a,color:#113
  class eu,apac boundary
  click scheduler callback "Inspect scheduler"
```

~~~mermaid theme=neutral
mindmap
  root((Atlas knowledge))
    Product
      (Roadmap)
      [Research]
        λ calculus
        Unicode 日本語
    Operations
      {{Reliability}}
        On-call
        Postmortems:::urgent
          ::icon(fa fa-fire)
    Community
      Contributors 🚀
~~~

```mermaid
pie showData
  %% Capacity allocation
  title Regional traffic share
  "Europe" : 42
  "Asia Pacific 東京" : 33
  "Americas" : 21
  "Experiments 🚀" : 4
```

```mermaid
quadrantChart
  title Migration candidates
  x-axis Low effort --> High effort
  y-axis Low impact --> High impact
  quadrant-1 Strategic bets
  quadrant-2 Quick wins
  quadrant-3 Defer
  quadrant-4 Maintenance
  Replace cache: [0.2, 0.8]
  Rewrite ledger: [0.9, 0.9]
  Refresh icons: [0.1, 0.3]
```

```mermaid
requirementDiagram
  requirement availability {
    id: SLO-001
    text: Serve 99.95 percent of requests
    risk: high
    verifymethod: analysis
  }
  performanceRequirement latency {
    id: PERF-007
    text: Complete checkout within 300 ms
    risk: medium
    verifymethod: test
  }
  element gateway {
    type: service
    docref: adr-042
  }
  gateway - satisfies -> availability
  latency <- verifies - gateway
```

```mermaid
sequenceDiagram
  autonumber
  title: Checkout orchestration 🚀
  box transparent Browser tier
    actor C as Customer
  end
  box rgb(235,245,255) Platform
    participant A as API
    participant L as Ledger
  end
  C->>+A: POST /orders
  Note right of A: Validate café basket
  critical Reserve inventory
    A->>L: reserve(items)
    L-->>A: reservation
  option Inventory unavailable
    L--xA: rejected
  end
  alt Payment accepted
    A-->>C: 202 Accepted
  else Payment rejected
    A-->>C: 402 Required
  end
  loop Until dispatched
    A-)L: poll status
  end
  deactivate A
```

::: mermaid
stateDiagram-v2
  direction LR
  [*] --> Idle : boot
  state "Waiting for work" as Idle
  Idle --> Running : dequeue
  state Running {
    [*] --> Validating
    Validating --> Charging : valid
    Charging --> Packing : paid
    Packing --> [*]
  }
  state fork_state <<fork>>
  Running --> fork_state
  fork_state --> Audit
  fork_state --> Notify
  state join_state <<join>>
  Audit --> join_state
  Notify --> join_state
  join_state --> Complete
  note right of Running
    Spans multiple lines with 東京.
    The rule closes at end note 🚀.
  end note
  Complete --> [*]
:::

```mermaid
journey
  title Operator investigates an alert
  section Detection
  Receive page: 2: On-call, Pager
  Open dashboard: 4: On-call
  section Diagnosis
  Correlate traces: 5: On-call, Observability
  Reproduce naïve input: 3: On-call, Support
  section Resolution
  Deploy fix 🚀: 5: Release bot, On-call
```

```mermaid
xychart-beta horizontal
  title "Weekly throughput 東京"
  x-axis "Week" ["W1", "W2", "W3", "W4"]
  y-axis "Requests (millions)" 0 --> 12.5
  bar [5, 7.5, 9, 11]
  line [4.5, 7, 9.5, 12]
```

Each fence closes before the next Markdown section so embedded parser state cannot leak between diagram families.
