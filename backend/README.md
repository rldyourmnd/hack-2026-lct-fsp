# Backend

The Rust service that replaces the mock behind the same `/api/v1` paths, built as a hexagonal architecture.
[`Dockerfile`](Dockerfile) builds the production API image used by `docker compose up --build`. PostgreSQL
persists accounts, sessions, profiles, sources, processing jobs and the matching index.

## Layout

| Crate | Package | Role |
| --- | --- | --- |
| `crates/domain` | `fsp-domain` | Domain model: entities, value objects and rules, no I/O and no framework. Holds the contract vocabulary (`Grade` with its order, `Specialization`, `Role`) |
| `crates/application` | `fsp-application` | Use cases and outbound ports for persistence, verification, GitHub, FSP and LLM analysis |
| `crates/http` | `fsp-http` | Inbound adapter on axum: routing, origin check, `X-Request-Id`, the error envelope `{ code, message, requestId, details }`, `GET /health`, `404 not_found` for unknown routes |
| `crates/storage-postgres` | `fsp-storage-postgres` | PostgreSQL adapter on `sqlx`/`PgPool`: embedded migrations and the synthetic demo seed |
| `crates/integration-github` | `fsp-integration-github` | Client of the GitHub insights service and GitHub OAuth identity (an optional ownership proof) |
| `crates/github-insights` | `fsp-github-insights` (binary `fsp-github-insights`) | Separate loopback service: analyzes public GitHub work in the background without a token (search, REST, blobless clones) and computes the documented portrait and heuristics ([GitHub insights](../docs/github-insights.md)) |
| `crates/integration-fsp-synthetic` | `fsp-integration-fsp-synthetic` | Bounded HTTP adapter and separate fictional FSP simulator |
| `crates/integration-bifrost` | `fsp-integration-bifrost` | Optional Bifrost adapter: résumé fields, assessment help, the estimate review (deep reasoning) and repository-to-competition matching |
| `crates/integration-email` | `fsp-integration-email` | Resend delivery with random codes and bounded HTTP; fixed-code delivery for local demo mode |
| `crates/integration-pdf` | `fsp-integration-pdf` | Bounded digital-text extraction through isolated Poppler processes |
| `crates/integration-interview` | `fsp-integration-interview` | Authenticated internal interview gateway with a 50-second HTTP deadline |
| `crates/interview-service` | `fsp-interview-service` | PostgreSQL graph-guided interview service; prerequisite traversal, optional Bifrost and offline rubric |
| `crates/server` | `fsp-server` (binary `fsp-career-api`) | Composition root: configuration, wiring, graceful shutdown on SIGINT/SIGTERM |

**Dependency rule:** `domain` ← `application` ← adapters (`http`, storage, integrations) ← `server`. The domain
depends on no other crate of the project and no framework; the application depends only on the domain and owns
the port traits; adapters depend inward; only `server` knows the concrete adapters. Cargo dependencies enforce the
direction.

## Toolchain

Rust 1.99.0 (`rust-toolchain.toml`), edition 2024, resolver 3. Workspace `Cargo.toml` declares dependency
requirements; `Cargo.lock` records resolved versions. Do not infer installed versions from a historical
"latest" list. `unsafe` is forbidden and clippy warnings fail the build. SQLx embeds migrations;
`storage-postgres/build.rs` makes migration-only changes invalidate the build cache.

## Run and check

```sh
just db-up                           # local PostgreSQL on 127.0.0.1:5434 (docker compose service)
cd backend
cargo run -p fsp-server              # http://127.0.0.1:8080, or `just backend` from the root
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked      # `just backend-check` runs all three
```

Storage tests and the `server/tests/contract.rs` suite run against real PostgreSQL: each test creates an
isolated `fsp_test_<timestamp>_<uuid>` database through the admin URL in `FSP_TEST_DATABASE_URL` (default
`postgres://fsp:fsp@127.0.0.1:5434/postgres`, the `just db-up` service). Idle test databases are reaped on the
next run without forcibly disconnecting an active test.

| Variable | Default | Meaning |
| --- | --- | --- |
| `HOST` | `127.0.0.1` | Bind address |
| `PORT` | `8080` | Bind port |
| `FSP_ALLOWED_ORIGINS` | `http://127.0.0.1:5173,http://localhost:5173` | Comma-separated browser origins accepted on requests with `Origin`; others get `403 origin_not_allowed` |
| `DATABASE_URL` | `postgres://fsp:fsp@127.0.0.1:5434/fsp` | PostgreSQL connection; migrations and the demo seed run on startup |
| `FSP_SIMULATOR_URL` | `http://127.0.0.1:4020` | Private fictional participant-results provider; start with `just fsp-simulator` |
| `FSP_GITHUB_INSIGHTS_URL` | unset | Enables GitHub connection through the insights service (`just github-insights`, port 4030); the service needs `git` and no token |
| `FSP_AUTH_MODE` | `demo` | `demo` uses a fixed local code; `resend` sends real random codes except for the three disclosed seeded demo addresses. Unknown values abort startup |
| `RESEND_API_KEY` | unset | Required in `resend` mode; use a sending-only key restricted to the sending domain |
| `FSP_EMAIL_FROM` | unset | Required bare sender address, e.g. `login@auth.nddev.link` |
| `FSP_AUTH_KEY` | unset | Required in `resend` mode: 32 random bytes as 64 hex characters; HMAC protects codes and sessions |
| `FSP_DEMO_EMAIL_CODE` | `123456` | Verification code for demo mode and the three exact seeded addresses in Resend mode |
| `FSP_SECURE_COOKIE` | unset | `1` adds `Secure` to the session cookie (set it behind HTTPS) |
| `FSP_PUBLIC_WEB_URL` | `http://127.0.0.1:5173` | Browser URL used after OAuth callbacks |
| `FSP_STATIC_DIR` | unset | Serve a built SPA from this directory and use its `index.html` as the non-API fallback |
| `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET` | unset | Enable GitHub OAuth when both are non-empty; `GITHUB_CALLBACK_URL` defaults to `http://127.0.0.1:8080/api/v1/integrations/github/callback` |
| `BIFROST_CHAT_URL` | unset | Enables Bifrost analysis when non-empty; `BIFROST_API_KEY` is optional and `BIFROST_MODEL` defaults to `openai/gpt-5-mini` |
| `FSP_INTERVIEW_URL`, `INTERVIEW_API_TOKEN` | unset | Enable the authenticated internal interview adapter; see [interview configuration](../docs/interview-service.md) |
| `FSP_TTS_PROVIDER` | `browser` | `xai` or `yandex` enables server-side neural speech for the current interview question; unset/`browser` preserves browser speech synthesis |
| `XAI_API_KEY`, `XAI_TTS_VOICE` | unset, `eve` | Required key and optional voice when `FSP_TTS_PROVIDER=xai` |
| `YANDEX_SPEECHKIT_API_KEY` | unset | Required service-account API key with the SpeechKit TTS scope when `FSP_TTS_PROVIDER=yandex`; voice defaults to `marina` |
| `FSP_PDF_IMPORT` | unset | `1` enables and probes the isolated PDF adapter; usable import also requires Bifrost |
| `FSP_GRADE_CHANGE_COOLDOWN_DAYS` | `90` | Successful grade-change cooldown, 0–365; invalid values use the default |

The [private CI service](../infra/ci/README.md) runs these checks with PostgreSQL and the real systemd PDF probe
for canonical PRs, and gates integration/deployment with `fsp/backend`.

## Contract and implemented boundaries

| Source | What it defines |
| --- | --- |
| [`../contracts/openapi.json`](../contracts/openapi.json) | Every operation except the `x-mock-only` `/__mock__` controls, which must not exist in production; index in [`API_OPERATIONS.md`](../contracts/API_OPERATIONS.md) |
| [`../docs/product/BUSINESS_RULES.md`](../docs/product/BUSINESS_RULES.md) | Accounts and access, profile visibility, consents, sources and skill states, assessment eligibility and outcomes, needs, matching and order, anonymous projection, shortlists, invitations, vacancies and applications, short tasks and freshness, disclosure, PDF, jobs |
| [`../contracts/API_INTEGRATION.md`](../contracts/API_INTEGRATION.md) | Session cookie, origin checks, validation, error envelope, idempotency, jobs, pagination, PDF, time, metadata, the cutover checklist |
| [`../mock-server`](../mock-server/README.md) | The rules as running code (`src/domain.ts`) and as HTTP tests (`test/mock-api.test.ts`), ported to `crates/server/tests/contract.rs` |

Rust implements the 79 application operations plus `/health`; `/__mock__` controls are absent. Sessions,
roles, tenant scoping and PostgreSQL persistence are real. Non-demo email ownership is verified by Resend; the three disclosed seed addresses and local `demo` mode retain a fixed code;
FSP uses a separate fictional HTTP provider. Knowledge tests are scored deterministically; no code executes. PDF text extraction uses a bounded, isolated
Poppler process and Bifrost proposes fields with validated page excerpts. The assessment assistant calls Bifrost
with the current saved answer and question; these replies never establish a grade. GitHub analysis runs in the insights service over public data only; ownership is proven by a per-candidate bio code or, when OAuth is configured, by a token whose `/user` login matches. The competence graph is
deterministic; repository-to-competition matching is an adapter port without a product call path.
`GET /api/v1/meta` reports configuration, not proof of a provider's availability or assessment validity.

Request budgets are nested: a call to the GitHub insights service takes at most 10 seconds (the analysis itself runs in the background) and a Bifrost completion inside a request has a 45-second ceiling (the background estimate review reasons with
`BIFROST_REASONING_EFFORT`, default `high`, for up to 330 seconds under Bifrost's 360-second provider timeout, one
review per candidate at a time);
interview evaluation and next-question generation share 45 seconds before persistence, inside a 50-second gateway deadline;
the HTTP handler has 55 seconds, inside both nginx proxies' 60-second read deadline. This leaves room
for storage and the contract error envelope. A provider timeout remains a visible integration failure;
no request falls back to synthetic data. HTTP timeout regressions use Tokio virtual time.

Five idempotent commands live in `application/src/service/commands.rs`. The key, domain effects and serialized
response share one transaction; failed work rolls back. Same-key callers serialize in PostgreSQL and replay the
stored status/body. Finished responses expire after one day; legacy unresolved claims fail closed.
Verification failures commit their attempt counter; live challenges share an email budget. Profile writes use
revision guards, assessment/source writes lock the candidate, and disclosure recomputation serializes per
candidate/company. Shortlist mutations serialize before reading their aggregate.

`sources.rs` performs bounded integration I/O before a short transaction, then atomically saves source evidence
and its job. It checks consent and source freshness again before applying sync results. FSP jobs finish inline. A
GitHub analysis runs in the insights service, so its job stays `running` with the source's `activeJobId`; each
poll asks the service and applies a finished report in one transaction, restarts an analysis the service forgot and
fails the job after 90 minutes. Jobs persist in PostgreSQL and remain scoped to their candidate; the API has no
durable worker queue. Estimate reviews run in process-local, coalesced background tasks and can be requested again
after failure or restart. GitHub evidence never creates or upgrades a verified grade.

`graphs.rs` constructs deterministic category → concept → technology graphs from the catalog and stored evidence.
Labels, levels and source summaries come from application facts; employer projections remove private evidence.
Graph structure is not an independent qualification assessment and does not require a model call.

Search ranking is deterministic but still operates on full candidate lists; list projections can repeat reads.
Single-profile hot paths use indexed lookups. This is appropriate to the present demo, not a measured capacity
claim. Introduce SQL pagination/batched projections only with preserved ranking/privacy tests and workload evidence.

## Running the frontend against the backend

```sh
cd frontend
bun install --frozen-lockfile
FSP_API_TARGET=http://127.0.0.1:8080 bun run dev   # app on http://127.0.0.1:5173, /api proxied to the backend
```

The development-only demo controls (actor switch, mock clock, demo mailbox) call `/__mock__` and will fail
against the backend; everything else uses the contract.

## Deployment

1. Pass the [backend compatibility checklist](../contracts/API_INTEGRATION.md#backend-compatibility-checklist), using the mock's
   HTTP tests as the list of cases.
2. Change the contract only through [`../contracts/README.md`](../contracts/README.md).
3. Deploy through [`../infra/README.md`](../infra/README.md#commands) and
   run the stand smoke suite (`bun run e2e:public` in `frontend/`).

## PDF import runtime

Enable with `FSP_PDF_IMPORT=1` and the configured Bifrost gateway. Install `bubblewrap` and `poppler-utils`.
Startup probes the exact sandbox and fails if it cannot start; there is no unsandboxed fallback. On Ubuntu,
`infra/provision.sh` installs `infra/apparmor/fsp-pdf-bwrap` to permit this executable's user namespaces without
relaxing the global userns policy. The service permits AF_NETLINK for setup of the isolated network namespace.
Docker import is opt-in and requires a host/runtime that permits nested unprivileged namespaces; do not grant
`--privileged` or disable seccomp to enable it. Export works independently.

Limits: 2 MiB upload, 20 pages, 64 KiB extracted text, two concurrent parser processes, 256 MiB address space,
3 CPU seconds and 5 elapsed seconds. Scans, password-protected, malformed and excessive documents fail visibly.
Files are streamed to the sandbox, not stored. The sandbox has an empty root with only read-only system runtime,
private temporary storage, and isolated user/process/network namespaces; no environment secrets are passed.
The model sees extracted text, never the current profile. Only supported fields with actual page excerpts are
accepted. Unknown fields preserve profile values; explicit month totals are not inferred from overlapping dates.
Preview confirmation checks processing consent and the original profile revision inside a candidate lock.
A stale preview returns `409 import_profile_changed`, so it cannot overwrite a later edit.

## Knowledge tests and FSP

[Knowledge tests](../docs/knowledge-tests.md) document immutable versions, deterministic scoring and legacy retention.
[FSP simulator](../docs/fsp-simulator.md) documents private HTTP fixtures, validation and source snapshot behavior.
Neither team participation nor AI output certifies individual professional experience.

### Runtime validation and schema rollback

`fsp-career-api --check-pdf-runtime` extracts an embedded PDF without database or LLM access. Normal startup
validates adapters, enabled PDF extraction and listener availability before opening/migrating PostgreSQL.
Use `infra/tests/pdf-runtime.sh` as root to test the binary under the actual API unit restrictions.

Storage permits unknown applied versions only above its embedded migration history, supporting rollback after
reviewed additive schema changes. Known checksums, unknown older versions and dirty migrations still fail.
A dedicated connection holds SQLx's advisory lock across validation/application and is closed on cancellation
or error, never returning a locked session to the pool. The release marker and guard are documented in the
[infrastructure runbook](../infra/README.md); they do not make destructive migrations safe to roll back.

### Transactional email

`crates/integration-email` implements the delivery port using Resend HTTPS, a
10-second deadline, no redirects, bounded responses and challenge-scoped provider
idempotency. Codes are six uniform CSPRNG digits, expire in 15 minutes and are
single-use. PostgreSQL serializes real-mail issuance per email (one/minute,
five/15 minutes) including failed provider calls. The cumulative five-wrong-code
budget remains in force across requests. Local fixed-code deliveries do not send
mail and do not spend the sending quota. In Resend mode this exception is restricted to
`alexey@example.com`, `hr@sever-tech.example` and `hr@second-company.example`; their disclosed code is not mailbox ownership proof.

Challenge-bound HMAC-SHA256 prevents a database dump from revealing six-digit
codes. Session digests have a separate namespace. Enabling or rotating
`FSP_AUTH_KEY` invalidates earlier sessions/codes without deleting accounts.
Never revert to demo authentication on the public stand. Provider acceptance is
logged by delivery ID, without addresses, keys or codes. Monitor delivery/bounce
status in Resend; a successful send response alone does not establish delivery.
`meta.capabilities.realEmail` controls the frontend's real-mail/demo guidance.
