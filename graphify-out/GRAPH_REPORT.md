# Graph Report - .  (2026-07-15)

## Corpus Check
- cluster-only mode — file stats not available

## Summary
- 1258 nodes · 2521 edges · 77 communities (69 shown, 8 thin omitted)
- Extraction: 98% EXTRACTED · 2% INFERRED · 0% AMBIGUOUS · INFERRED: 39 edges (avg confidence: 0.7)
- Token cost: 42,900 input · 3,504 output

## Graph Freshness
- Built from commit: `543daede`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- Database & Admin Stats
- Attestation Guard Middleware
- App Configuration
- Password & Account Management
- Router Test Setup
- Native Plugin API Definitions
- iOS Native Plugin (Swift)
- Capacitor Native Dependencies
- Game Profile & Leaderboard
- Native Plugin Package Config
- Coach Charts & Signals UI
- Provider Connections Data
- Design System Generator Script
- Web App Vault UI
- Ad Campaign Strategy Docs
- Design System Reasoning Engine
- Android Native Plugin (Kotlin)
- Device Capability Detection
- Design System Master Doc
- Sync Document Storage
- On-device AI & Billing UI
- UI Design Skill Workflow
- Auth & Device Registration
- Game API Handlers
- Sync API Handlers
- Web API Client
- Design Search Core Engine
- BM25 Ranking Algorithm
- TypeScript Config
- App Store Readiness Skill
- AI Proxy API
- Professional UI Rules
- UI Quick Reference Guide
- Project README Overview
- OAuth State Tokens
- App Store Audit Doc
- App Boot & Onboarding
- App Navigation & Routing
- Go-Live Runbook
- Nonce Cache
- Run Lifeline Skill
- Session Tokens
- Token Vault Encryption
- Audit Log Signing
- Admin Auth Handlers
- WebSocket Streaming
- App Review Notes
- Admin Dashboard UI
- Firebase Waitlist Signup
- Native Shells Build Notes
- Privacy & Launch Checklist
- App Store Listing Copy
- Native Bridge JS
- Infrastructure Connectors Map
- Insights Config API
- Launch Checklist Doc
- Waitlist Site Setup
- Project Workflow Doc
- Native Plugin README
- Security Review Doc
- Sound Effects Module
- Browser Driver Script
- Data Validation Script
- Service Worker Caching
- Search Output Formatter
- Health Check Endpoint
- Attribution Doc
- App Icons & Splash
- Load Test Script
- PGO Build Script

## God Nodes (most connected - your core abstractions)
1. `AppError` - 179 edges
2. `AppState` - 80 edges
3. `MockDatabase` - 48 edges
4. `PostgresDatabase` - 34 edges
5. `VerifiedDevice` - 28 edges
6. `SyncDocument` - 23 edges
7. `LifelineNativePlugin` - 22 edges
8. `Account` - 22 edges
9. `GameProfile` - 20 edges
10. `create_test_state()` - 20 edges

## Surprising Connections (you probably didn't know these)
- `create_test_state()` --references--> `MockDatabase`  [EXTRACTED]
  tests/integration_tests.rs → src/db/mod.rs
- `create_test_state_with_env()` --references--> `MockDatabase`  [EXTRACTED]
  tests/integration_tests.rs → src/db/mod.rs
- `register_device_with_token()` --references--> `MockDatabase`  [EXTRACTED]
  tests/integration_tests.rs → src/db/mod.rs
- `test_account_deletion_erases_account_and_device()` --calls--> `create_router()`  [INFERRED]
  tests/integration_tests.rs → src/routes/mod.rs
- `test_account_signin_signup_flow()` --calls--> `create_router()`  [INFERRED]
  tests/integration_tests.rs → src/routes/mod.rs

## Import Cycles
- None detected.

## Communities (77 total, 8 thin omitted)

### Community 0 - "Database & Admin Stats"
Cohesion: 0.05
Nodes (54): DecodeError, From, HashMap, Mutex, ProviderConnectionRecord, Send, admin_stats(), AdminStats (+46 more)

### Community 1 - "Attestation Guard Middleware"
Cohesion: 0.05
Nodes (82): Bytes, Client, attest_guard(), Arc, Body, Next, Request, Response (+74 more)

### Community 2 - "App Configuration"
Cohesion: 0.06
Nodes (37): Certificate, ConfigError, Default, AdminConfig, ai(), AiBudget, AiConfig, AppConfig (+29 more)

### Community 3 - "Password & Account Management"
Cohesion: 0.09
Nodes (40): hash_password(), roundtrip_and_reject(), Result, Vec, salts_differ_per_hash(), delete_account_and_data(), get_account_by_email(), get_account_by_oauth() (+32 more)

### Community 4 - "Router Test Setup"
Cohesion: 0.12
Nodes (39): SocketAddr, create_router(), harden_and_cache(), Arc, Body, Next, Request, Response (+31 more)

### Community 5 - "Native Plugin API Definitions"
Cohesion: 0.08
Nodes (3): LifelineNativePlugin, LifelineNative, LifelineNativeWeb

### Community 6 - "iOS Native Plugin (Swift)"
Cohesion: 0.08
Nodes (18): ASAuthorization, ASAuthorizationController, ASAuthorizationControllerDelegate, ASAuthorizationControllerPresentationContextProviding, ASPresentationAnchor, AuthenticationServices, Capacitor, CAPPlugin (+10 more)

### Community 7 - "Capacitor Native Dependencies"
Cohesion: 0.06
Nodes (33): @capacitor/android, @capacitor/assets, @capacitor/cli, @capacitor/device, @capacitor/haptics, @capacitor/ios, @capacitor/local-notifications, @capacitor/status-bar (+25 more)

### Community 8 - "Game Profile & Leaderboard"
Cohesion: 0.10
Nodes (21): NaiveDate, get_game_profile(), is_handle_taken(), leaderboard(), Option, PgPool, Result, Uuid (+13 more)

### Community 9 - "Native Plugin Package Config"
Cohesion: 0.07
Nodes (29): src, capacitor, android, ios, description, devDependencies, @capacitor/core, rollup (+21 more)

### Community 10 - "Coach Charts & Signals UI"
Cohesion: 0.12
Nodes (25): anecdoteStats(), conductorMode(), conductorTonePrompt(), domainForSignal(), msgHtml(), personalShape(), userFocus(), viewCoach() (+17 more)

### Community 11 - "Provider Connections Data"
Cohesion: 0.13
Nodes (21): delete_provider_connection(), get_encrypted_refresh_token(), list_provider_connections(), Option, PgPool, Result, Uuid, Vec (+13 more)

### Community 12 - "Design System Generator Script"
Cohesion: 0.12
Nodes (24): ansi_ljust(), _detect_page_type(), format_ascii_box(), format_markdown(), format_master_md(), format_page_override_md(), generate_design_system(), _generate_intelligent_overrides() (+16 more)

### Community 13 - "Web App Vault UI"
Cohesion: 0.12
Nodes (21): fillTemplate(), FOCI, FREE_ENTITLEMENTS, generateDailyAnecdote(), I, maybeSendDailyNotification(), oneSentence(), PIGMENT (+13 more)

### Community 14 - "Ad Campaign Strategy Docs"
Cohesion: 0.09
Nodes (20): 0. Campaign map (what to run where), 1. Apple Search Ads (highest priority), 2. Google Ads — Search, 3. Meta (Instagram + Facebook), 4. TikTok / Reels / Shorts (organic + paid share the same scripts), 5. Reddit (traffic → `/compare`), 6. App Store "Why switch" — long description block, 7. UTM + measurement (so you can actually read results) (+12 more)

### Community 15 - "Design System Reasoning Engine"
Cohesion: 0.14
Nodes (11): DesignSystemGenerator, Find matching reasoning rule for a category., Apply reasoning rules to search results., Select best matching result based on priority keywords., Extract results list from search result dict., Generate complete design system recommendation.          variance/motion/density, Bucket a 1-10 dial value into its tier config. Returns None if value is None., Generates design system recommendations from aggregated searches. (+3 more)

### Community 16 - "Android Native Plugin (Kotlin)"
Cohesion: 0.19
Nodes (3): LifelineNativePlugin, Plugin, PluginCall

### Community 17 - "Device Capability Detection"
Cohesion: 0.19
Nodes (14): api, computeTier(), detectCores(), detectPlatform(), detectRamGb(), eligibleModels(), inferenceBackends(), nativeProfile() (+6 more)

### Community 18 - "Design System Master Doc"
Cohesion: 0.11
Nodes (17): Additional Forbidden Patterns, Anti-Patterns (Do NOT Use), Buttons, Cards, Color Palette, Component Specs, Design System Master File, Global Rules (+9 more)

### Community 19 - "Sync Document Storage"
Cohesion: 0.24
Nodes (16): get_document_history(), get_latest_document(), list_latest_documents_by_type(), Option, PgPool, Result, Uuid, Vec (+8 more)

### Community 20 - "On-device AI & Billing UI"
Cohesion: 0.24
Nodes (18): boardRows(), can(), cap(), confetti(), confirmDeleteAccount(), downloadOnDeviceModel(), esc(), offlineBanner() (+10 more)

### Community 21 - "UI Design Skill Workflow"
Cohesion: 0.12
Nodes (16): Before Delivering App UI, Example Workflow, If a search returns 0 results, Output Formats, Rule Categories by Priority, Running the search tool, Step 1: Analyze User Requirements, Step 2: Generate Design System (REQUIRED for new pages/projects) (+8 more)

### Community 22 - "Auth & Device Registration"
Cohesion: 0.34
Nodes (16): assert_handler(), AssertRequest, challenge_handler(), dev_session_handler(), DevSessionRequest, register_device_and_issue_token(), Arc, Json (+8 more)

### Community 23 - "Game API Handlers"
Cohesion: 0.30
Nodes (16): game_config_handler(), get_profile_handler(), leaderboard_handler(), LeaderboardQuery, profile_json(), Arc, Extension, Json (+8 more)

### Community 24 - "Sync API Handlers"
Cohesion: 0.34
Nodes (16): get_document_handler(), get_document_history_handler(), list_documents_by_type_handler(), Arc, Extension, Json, Option, Path (+8 more)

### Community 25 - "Web API Client"
Cohesion: 0.20
Nodes (13): account, accountCall(), b64, connect(), del(), deviceCrypto, emit(), get() (+5 more)

### Community 26 - "Design Search Core Engine"
Cohesion: 0.22
Nodes (14): detect_domain(), _domain_keywords(), _load_csv(), _load_product_keywords(), Load CSV and return list of dicts, with mtime-based caching., Core search function using BM25. Returns (results, bm25_or_none)., Nearest known vocabulary terms for a query that returned 0 hits,     so the call, Auto-detect the most relevant domain from query.      Matches are weighted by ke (+6 more)

### Community 27 - "BM25 Ranking Algorithm"
Cohesion: 0.16
Nodes (10): BM25, _get_bm25(), _normalize(), Apply synonym substitution before tokenizing., BM25 ranking algorithm for text search, Lowercase, normalize synonyms, split, remove punctuation, filter stopwords, Build BM25 index from documents, Score all documents against query (+2 more)

### Community 28 - "TypeScript Config"
Cohesion: 0.13
Nodes (14): compilerOptions, declaration, esModuleInterop, lib, module, moduleResolution, outDir, sourceMap (+6 more)

### Community 29 - "App Store Readiness Skill"
Cohesion: 0.14
Nodes (13): Agent Roster & Quick Dispatch, COMMERCE — IAP Strategist, DESIGNER — HIG Expert, FIXER — Rejection Recovery, iOS App Store Readiness Skill, Launch Gate (HARD GATE before Ship), MENTOR — Teaching Partner, METADATA — ASO Specialist (+5 more)

### Community 30 - "AI Proxy API"
Cohesion: 0.35
Nodes (13): ai_proxy_handler(), AiProxyRequest, call_anthropic(), call_openai_compatible(), local_models_handler(), policy_matrix_handler(), Arc, Extension (+5 more)

### Community 31 - "Professional UI Rules"
Cohesion: 0.15
Nodes (12): Accessibility, Common Rules for Professional UI + Pre-Delivery Checklist, Icons & Visual Elements, Interaction, Interaction (App), Layout, Layout & Spacing, Light/Dark Mode (+4 more)

### Community 32 - "UI Quick Reference Guide"
Cohesion: 0.15
Nodes (12): 10. Charts & Data (LOW), 1. Accessibility (CRITICAL), 2. Touch & Interaction (CRITICAL), 3. Performance (HIGH), 4. Style Selection (HIGH), 5. Layout & Responsive (HIGH), 6. Typography & Color (MEDIUM), 7. Animation (MEDIUM) (+4 more)

### Community 33 - "Project README Overview"
Cohesion: 0.15
Nodes (13): API surface (v1), Architecture, Build, run, and drive it, Deploying to the App Store & Google Play, Highlights, 🫀 Lifeline, Project structure, Quick start (+5 more)

### Community 34 - "OAuth State Tokens"
Cohesion: 0.36
Nodes (12): create_state_token(), derive_oauth_state_key(), OAuthStatePayload, Key, Result, String, Uuid, test_state_token_expired_rejected() (+4 more)

### Community 35 - "App Store Audit Doc"
Cohesion: 0.15
Nodes (12): 1. Privacy manifest — FIXED, 2. HealthKit Info.plist usage strings, 3. Visible medical disclaimer — **Guideline 1.4.1 / 5.1.3**, 4. Reviewer access — **Guideline 2.1**, 5. Labels ↔ manifest ↔ reality must match — **Guideline 5.1.1(i)**, 🔴 BLOCKING, ✅ CLEAR — verified in the code, Launch gate status (+4 more)

### Community 36 - "App Boot & Onboarding"
Cohesion: 0.21
Nodes (13): onConnection(), status(), applyTheme(), authGate(), bootSequence(), main(), onboarding(), refreshArena() (+5 more)

### Community 37 - "App Navigation & Routing"
Cohesion: 0.23
Nodes (13): applyConductor(), MORE_ROUTES, openMoreSheet(), paintProviders(), paintTabbar(), refreshSources(), render(), routeId() (+5 more)

### Community 38 - "Go-Live Runbook"
Cohesion: 0.17
Nodes (11): Android → `.aab` for Google Play  (buildable on Linux/Mac/Windows), If you want me to do more from here, iOS → `.ipa` for the App Store  (requires a Mac + Xcode — Apple's rule), Lifeline — go-live runbook, Step 1 — Deploy the engine (your host), Step 2 — Your database (Supabase), Step 3 — Your Stripe (billing), Step 4 — Build the store binaries (+3 more)

### Community 39 - "Nonce Cache"
Cohesion: 0.27
Nodes (7): NonceCache, Cache, Result, Self, String, test_invalid_nonce(), test_nonce_cache()

### Community 40 - "Run Lifeline Skill"
Cohesion: 0.18
Nodes (10): Build, Driving on-device AI (premium-device path), Gotchas, Prerequisites, Run (agent path), Run (API smoke, no browser), Run (human path), Run Lifeline (+2 more)

### Community 41 - "Session Tokens"
Cohesion: 0.38
Nodes (10): create_session_token(), Key, Result, String, Uuid, SessionTokenPayload, test_expired_session_token(), test_session_token_lifecycle() (+2 more)

### Community 42 - "Token Vault Encryption"
Cohesion: 0.36
Nodes (10): decrypt_token(), derive_token_vault_key(), encrypt_token(), LessSafeKey, Result, String, Vec, test_decrypt_tampered_blob_fails() (+2 more)

### Community 43 - "Audit Log Signing"
Cohesion: 0.31
Nodes (10): AuditLogEntry, AuditRecordFields, compute_signature(), derive_audit_key(), DateTime, Key, String, Utc (+2 more)

### Community 44 - "Admin Auth Handlers"
Cohesion: 0.27
Nodes (9): admin_authorized(), admin_stats_handler(), require_admin(), Arc, HeaderMap, Json, Result, State (+1 more)

### Community 45 - "WebSocket Streaming"
Cohesion: 0.24
Nodes (10): handle_socket(), Arc, HeaderMap, Response, Result, State, Uuid, ws_upgrade_handler() (+2 more)

### Community 46 - "App Review Notes"
Cohesion: 0.20
Nodes (10): Account deletion (Guideline 5.1.1(v)), AI coach, Daily notifications (opt-in), Health data, How to sign in, Leaderboard content safety, Notes for app review (Apple / Google), On-device AI (optional, premium devices) (+2 more)

### Community 47 - "Admin Dashboard UI"
Cohesion: 0.31
Nodes (9): dash, esc(), existing, fmt(), loadStats(), login, render(), show() (+1 more)

### Community 48 - "Firebase Waitlist Signup"
Cohesion: 0.22
Nodes (6): btn, emailEl, form, msg, firebaseConfig, NOTE: these values are NOT secret. The Web API key just identifies your

### Community 49 - "Native Shells Build Notes"
Cohesion: 0.22
Nodes (9): Account deletion (Guideline 5.1.1(v)), Build steps, Daily notifications (opt-in), iOS specifics, Lifeline native shells (iOS + Android), On-device AI (optional, premium devices), Store assets, ⚠️ Subscriptions in the store builds (+1 more)

### Community 50 - "Privacy & Launch Checklist"
Cohesion: 0.22
Nodes (4): Apple — App Privacy ("nutrition label"), Google Play — Data safety, One-line stance for both reviews, Privacy label answers

### Community 51 - "App Store Listing Copy"
Cohesion: 0.25
Nodes (8): Categories, Full description, Keywords (App Store, ≤100 chars), Lifeline — store listing copy, Name, Privacy policy URL, Short description (Google Play, ≤80 chars), Subtitle (App Store, ≤30 chars)

### Community 52 - "Native Bridge JS"
Cohesion: 0.57
Nodes (7): cap(), define(), Device(), installNativeBridges(), LN(), LocalNotifications(), plugins()

### Community 53 - "Infrastructure Connectors Map"
Cohesion: 0.29
Nodes (6): 🚫 Deliberately unused (by design, not omission), 🗺 Platform map (fast + cheap), Provisioned infrastructure & connector map, 💰 Revenue model (already wired), ✅ Stripe — revenue (provisioned), ✅ Supabase — production Postgres (provisioned, $0/month)

### Community 54 - "Insights Config API"
Cohesion: 0.38
Nodes (6): insights_config_handler(), Arc, Json, Result, State, Value

### Community 55 - "Launch Checklist Doc"
Cohesion: 0.29
Nodes (7): 1. Backend to production, 2. Stripe (web subscriptions), 3. Accounts, privacy & data rights (App Review blockers), 4. Listings, 4. Store binaries (see `native/README.md`), 5. Final gates, Lifeline launch checklist

### Community 56 - "Waitlist Site Setup"
Cohesion: 0.29
Nodes (6): Files, Lifeline waitlist site, Notes, Prefer not to use Firebase?, Seeing the signups, Setup (~10 minutes, one time)

### Community 57 - "Project Workflow Doc"
Cohesion: 0.33
Nodes (5): Deploy & docs, Everyday commands, gstack (recommended AI workflow), Lifeline — project workflow, Project skills (in `.claude/skills/`)

### Community 58 - "Native Plugin README"
Cohesion: 0.33
Nodes (4): Build, Capabilities to enable (Xcode) / permissions (Android), lifeline-native (Capacitor plugin), What's implemented vs. an integration point

### Community 59 - "Security Review Doc"
Cohesion: 0.33
Nodes (5): Findings, 🔴 FIXED — Payment tier could be spoofed via the checkout URL (A04 / A08), Lifeline — security weak-spot review, Residual notes (accept / monitor), Verified clean (no action needed)

### Community 60 - "Sound Effects Module"
Cohesion: 0.47
Nodes (5): ac(), armGlobalSounds(), env(), sound, tone()

### Community 62 - "Data Validation Script"
Cohesion: 0.83
Nodes (3): _check_file(), main(), _read_rows()

## Knowledge Gaps
- **249 isolated node(s):** `errors`, `name`, `version`, `private`, `description` (+244 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **8 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `AppError` connect `Database & Admin Stats` to `Attestation Guard Middleware`, `App Configuration`, `Password & Account Management`, `OAuth State Tokens`, `Nonce Cache`, `Game Profile & Leaderboard`, `Session Tokens`, `Token Vault Encryption`, `Provider Connections Data`, `Admin Auth Handlers`, `WebSocket Streaming`, `Sync Document Storage`, `Auth & Device Registration`, `Game API Handlers`, `Insights Config API`, `Sync API Handlers`, `AI Proxy API`?**
  _High betweenness centrality (0.150) - this node is a cross-community bridge._
- **Why does `AppState` connect `Attestation Guard Middleware` to `Database & Admin Stats`, `App Configuration`, `Password & Account Management`, `Router Test Setup`, `Nonce Cache`, `Admin Auth Handlers`, `WebSocket Streaming`, `Sync Document Storage`, `Auth & Device Registration`, `Game API Handlers`, `Insights Config API`, `Sync API Handlers`, `AI Proxy API`?**
  _High betweenness centrality (0.057) - this node is a cross-community bridge._
- **Why does `AppConfig` connect `App Configuration` to `Attestation Guard Middleware`, `Nonce Cache`?**
  _High betweenness centrality (0.030) - this node is a cross-community bridge._
- **What connects `errors`, `name`, `version` to the rest of the system?**
  _249 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Database & Admin Stats` be split into smaller, more focused modules?**
  _Cohesion score 0.05067064083457526 - nodes in this community are weakly interconnected._
- **Should `Attestation Guard Middleware` be split into smaller, more focused modules?**
  _Cohesion score 0.054553264604811 - nodes in this community are weakly interconnected._
- **Should `App Configuration` be split into smaller, more focused modules?**
  _Cohesion score 0.06298701298701298 - nodes in this community are weakly interconnected._