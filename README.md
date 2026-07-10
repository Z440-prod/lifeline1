# 🛡️ Antigravity Engine
### Secure, Zero-Knowledge E2EE Backend for Lifeline iOS

[![Rust](https://img.shields.io/badge/rust-1.75%2B-blue.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Build Status](https://img.shields.io/badge/status-production--ready-success.svg)]()

**Antigravity** is a high-performance, security-first Rust backend framework engine designed specifically to power the **Lifeline iOS** application. Built on top of the Axum web framework and Tokio runtime, it implements state-of-the-art cryptographic validation and client privacy protections.

---

## ✨ Features

### 1. 🔒 Apple App Attest & Assertion
Protects endpoints against botnets, malicious scripts, and cost abuse.
*   **Hardware-Backed Identity:** Integrates Apple App Attest to cryptographically verify device and app integrity before allowing registration.
*   **Replay Protection:** Enforces strict monotonic counter verification for subsequent assertions, preventing replay attacks.
*   **Cryptographic Key Registration:** Securely registers EC P-256 client public keys in PostgreSQL database.

### 2. 🔑 Zero-Knowledge E2EE Document Sync
The backend is completely blind to user data.
*   **Blind Persistence:** Synchronizes user biometric data and documents using encrypted blobs.
*   **AEAD Validation:** Opaque payloads contain IV, ciphertext, and authentication tags encrypted on-device via Secure Enclave.
*   **Optimistic Concurrency:** Uses a PostgreSQL `SERIALIZABLE` transaction isolation level with automatic retry mechanisms to ensure conflict-free sync resolution.

### 3. 🤖 Anonymized AI Proxy
A privacy-first gateway to Claude 3.5 Sonnet.
*   **Metadata Stripping:** Completely strips client-identifying details (IP addresses, device IDs, browser user-agents, etc.) before calling upstream LLM services.
*   **Policy Matrix Enforcement:** Implements behavior model checks and clinical-first assistance prompts locally on the server.

### 4. 🔗 Health Platform Integrations
Connect Apple Health, Google Health Connect, and Whoop.
*   **On-Device Providers:** Apple HealthKit and Google Health Connect are read locally on the client — the server only records that a device authorized access, never the underlying data.
*   **Whoop OAuth2:** The one provider requiring a server-held credential. Refresh tokens are encrypted at rest with a key derived from the server secret (`crypto::token_vault`) and the authorize/callback flow is bound to the requesting device with a signed, short-lived `state` parameter (`crypto::oauth_state`).

### 5. 🧪 Doctor-Provided Lab Results
Uploaded through the same zero-knowledge E2EE sync pipeline as any other document — the server stores an opaque, client-encrypted blob tagged with a `document_type` category (e.g. `lab_result`) purely for UI grouping, and never sees test values or provider names.

### 6. ⚡ Production-Ready Performance & Security
*   **Rate Limiting:** IP-based rate limiting via a token bucket algorithm powered by `tower-governor`.
*   **Structured Errors:** Clean domain-specific error handling utilizing the `thiserror` and `anyhow` crates.
*   **Observability:** Out-of-the-box `/health` liveness checks and Prometheus scraping `/metrics` endpoints tracking request latencies.
*   **PGO Pipeline:** Profile-Guided Optimization scripts for 10-20% runtime performance enhancements.

---

## 🛠️ Tech Stack
*   **Runtime:** `tokio` (Async multi-threaded)
*   **Web Framework:** `axum` (Routing & Middleware)
*   **Database:** `sqlx` (Async PostgreSQL client with automatic migration engine)
*   **Crypto:** `ring` (HMAC, ECDSA signature verification), `x509-cert` / `der` (Certificate validation)
*   **Caching:** `moka` (TTL-based single-use nonce caching)

---

## 🚀 Getting Started

### 📋 Prerequisites
*   **Rust** (1.75+ or newer)
*   **PostgreSQL** (14+ for SERIALIZABLE advisory lock support)

### ⚙️ Local Development Setup

1.  **Clone the Repository:**
    ```bash
    git clone https://github.com/your-username/antigravity-engine.git
    cd antigravity-engine
    ```

2.  **Environment Variables Setup:**
    Copy the example configuration to `.env`:
    ```bash
    cp .env.example .env
    ```
    Edit `.env` to supply your local PostgreSQL database URL and development parameters.

3.  **Run Migrations & Start Server:**
    The engine runs database migrations automatically on startup:
    ```bash
    cargo run
    ```
    The server will start listening at `http://0.0.0.0:8443` (port configurable in `.env`).

4.  **Open the Lifeline web app:**
    The full product front end lives in `web/` and is served by the same binary —
    open **http://localhost:8443/** and you're in the app. In development the
    browser authenticates via `POST /auth/dev-session` (registers a real WebCrypto
    P-256 key for this browser, so vault documents are genuinely encrypted and
    signed client-side). The endpoint is hard-disabled outside
    `ENVIRONMENT=development`; on iOS hardware the native client uses Apple App
    Attest against the same API.

    > **TLS:** Antigravity speaks plain HTTP. In production it must sit behind a TLS-terminating
    > reverse proxy, load balancer, or platform ingress (e.g. nginx, Caddy, an AWS/GCP load
    > balancer) — it does not perform TLS termination itself.

---

## 📦 Production Builds & Profile-Guided Optimization (PGO)
To achieve maximum compiler optimization for your target architecture, utilize the PGO build pipeline:

1.  Build the instrumented binary:
    ```bash
    ./scripts/pgo_build.sh
    ```
2.  Run the server and perform a load test / representative workload.
3.  Re-run the script to compile the final optimized production binary.

---

## 📈 Monitoring
*   **Liveness Check:** `GET /health`
*   **Prometheus Exporter:** `GET /metrics`

---

## 📜 License
This project is licensed under the MIT License - see the LICENSE file for details.
