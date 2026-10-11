# FSP / Career

A reverse-hiring platform for the FSP 2026 special track. Candidates complete a questionnaire and a
versioned knowledge test; employers search anonymous profiles and send offers with a salary range.
Candidates disclose selected contacts to a company by accepting its invitation or applying to its vacancy.
Recurring company tasks provide a separate freshness signal.

Public stand: <https://nddev.link>. Email verification sends a single-use code from `login@auth.nddev.link` through Resend.
Rust and PostgreSQL persist platform data. Knowledge tests are scored deterministically; digital PDF import and
contextual assistance use isolated extraction and Bifrost. Seed candidates and the separate HTTP FSP provider
are explicitly fictional. A separate Rust service analyzes a candidate's public GitHub work on our server, without a
token, into a developer portrait and an explainable signal; GitHub OAuth setup is deferred. Capability flags describe configuration, not provider health.
Knowledge results do not certify professional experience. See [knowledge tests](docs/knowledge-tests.md),
[GitHub insights](docs/github-insights.md) and [FSP simulator](docs/fsp-simulator.md).
Candidates can also take a [graph-guided interview](docs/interview-service.md) with durable sessions, optional
browser speech and Bifrost evaluation. Interview evidence does not assign a platform grade.

## Repository

| Path | Contents |
| --- | --- |
| [frontend/](frontend/README.md) | React / TypeScript application, Russian interface |
| [contracts/](contracts/README.md) | Shared OpenAPI contract, error catalog and type generator |
| [mock-server/](mock-server/README.md) | Stateful development mock and executable behavior reference |
| [backend/](backend/README.md) | Hexagonal Rust API, PostgreSQL and integration adapters |
| [infra/](infra/README.md) | Stand provisioning, immutable releases, activation rollback and nginx |
| [docs/product/](docs/product/BUSINESS_RULES.md) | Implemented rules, requirements and organizer specification |
| [Current state](docs/current-state.md) | Verified behavior, remaining limits and release verification commands |
| [Stability audit](docs/stability-audit.md) | Historical session reconciliation and delivery evidence |
| [October 11 review](docs/stability-review-2026-10-11.md) | Recent session reconciliation, confirmed fixes and release verification |

Ownership and contribution rules: [AGENTS.md](AGENTS.md). Feature PRs target `danil` or `dima`;
[Private-server CI](infra/ci/README.md) checks and merges integration PRs into `dev`, then `main`.
Deployment ships verified main and runs the actual public browser journey; GitHub Actions is retired.

## Run

```sh
docker compose up --build       # http://localhost:8080; demo verification code 123456
docker compose down            # keeps PostgreSQL data in pg-data
```

The application binds to loopback by default (`FSP_BIND_IP` can explicitly change it); the API shares nginx's network namespace. PostgreSQL's development
port is bound to `127.0.0.1:5434`. Use `FSP_PORT=9090` for a different application port and `FSP_DB_PORT` for a different local database port.
`docker compose down --volumes` deliberately destroys local data; it is not an update or deployment step.

## Develop and verify

```sh
just install                   # pinned TypeScript dependencies via Bun
just mock                      # development mock on 127.0.0.1:4010
just dev                       # frontend on http://127.0.0.1:5173
just db-up                     # local PostgreSQL for Rust and integration tests
just fsp-simulator             # fictional provider on 127.0.0.1:4020 (separate terminal)
just backend                   # Rust API on 127.0.0.1:8080
just verify                    # contract, mock, frontend types/lint/tests/build
just backend-check             # rustfmt, clippy, unit and PostgreSQL HTTP/storage tests
just e2e                       # browser acceptance against the mock
```

The historical Python/FastAPI MVP and its documentation are preserved at tag `archive/dima-mvp`.
Inspect it with `git show archive/dima-mvp:README.md` or a separate detached worktree; it is not part of main.

The owner standard forbids backups, snapshots and redundant repository mirrors. Backup creation is disabled;
local branches synchronize only with canonical `upstream`. See [operations policy](infra/README.md#backups-and-recovery).
