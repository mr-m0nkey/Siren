# Siren 🔔

**Siren** is a small Rust service monitor that periodically pings configured services and sends status updates (via Telegram using `teloxide`). It currently performs HTTP health checks using a blocking Reqwest client; the project uses Tokio for orchestration.

---

## ✅ Features (current)

- Basic HTTP uptime checks
- Sends notifications to a Telegram chat via `teloxide`
- Configurable services via `config/services.yml`

---

## 💡 Quick start

1. Set up environment variables (create a `.env` file):

```bash
# .env
BOT_TOKEN=123:ABC...
CHAT_ID=123456789
```

2. Add services in `config/app.yml` (example):

```yaml
services:
  - name: Prowlarr
    host: "http://localhost:9696/health" # or whatever your health endpoint is
    service_type: Http
    enabled: true

  - name: Sonarr
    host: "http://localhost:8989/health"
    service_type: Http
    enabled: true
```

3. Run locally:

```bash
cargo run
```

The program will ping configured services and send messages to the Telegram chat.

---

## 📋 TODOs

These are prioritized tasks with suggested approaches and acceptance criteria.

1. Ability to work P2P with other agents to ping services and share load 🔁
   - Goal: Enable multiple Siren instances (agents) to coordinate which services they ping and share load.
   - Approaches: gossip protocol, simple leader election, or using a lightweight overlay (e.g., libp2p, or a central coordination service like etcd/consul).
   - Acceptance criteria:
     - Agents can discover each other automatically or via static peers config
     - A minimal protocol exists to assign responsibility for a service (e.g., hashing + lease) and reassign when an agent goes offline
     - Tests: integration test demonstrating two local agents split a list of 4 services and cover reassignment when one shuts down
   - Complexity: Medium–High

2. Ability to schedule pings ⏱️ [Done]
   - Goal: Add scheduling so each service can be pinged at a configured interval (cron-like or interval in seconds).
   - Approaches: use `tokio::time::interval`, or integrate a scheduler crate (e.g., `cron` or `tokio-cron-scheduler`).
   - Acceptance criteria:
     - Each service config can include `interval_seconds` (or cron expression)
     - Scheduling is resilient to panics and keeps running
     - Unit tests verifying interval adherence and that scheduled jobs run concurrently
   - Complexity: Low–Medium

3. Ability to ping TCP ports 🔌
   - Goal: Add TCP port checks (e.g., open TCP socket) in addition to HTTP checks.
   - Approaches: use `std::net::TcpStream::connect_timeout` inside `spawn_blocking` or use async `tokio::net::TcpStream` for non-blocking checks.
   - Acceptance criteria:
     - Add `ServiceType::Tcp` with `port` or include it in `host` (`host:port`)
     - TCP check reports UP when connect succeeds within timeout, DOWN otherwise
     - Tests: unit/integration test for an ephemeral TCP listener
   - Complexity: Low

3. Docker Deployment
   - Goal: Deploy to docker so it can be used in kubernetes or with docker-compose
   - Complexity: Low


4. Code Review
    - Goal: Clean up code, especially around async code.

---

## 🛠️ Contributing

Contributions welcome — open an issue first to discuss large changes.

- Fork the repo
- Create a feature branch
- Add tests and documentation for new features
- Open a pull request describing the change and the rationale

---
