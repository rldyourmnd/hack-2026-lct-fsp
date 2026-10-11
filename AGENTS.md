# Repository rules

The FSP reverse-hiring platform for the FSP 2026 special track: contract, mock, frontend, Rust backend and the demo
stand. Dima's first version, a Python (FastAPI) MVP, is tagged `archive/dima-mvp` and is not part of `main`.

## Map

| Path | Owner | Contents |
| --- | --- | --- |
| [`backend/`](backend/README.md) | Дима | Hexagonal Rust backend that implements the contract (PostgreSQL storage, GitHub/ФСП/Bifrost adapters; coverage tracked in [`docs/backend-implementation-checklist.md`](docs/backend-implementation-checklist.md)) |
| [`contracts/`](contracts/README.md) | shared | OpenAPI contract, error codes, generated operation index, TypeScript generator |
| [`mock-server/`](mock-server/README.md) | Данил | Stateful HTTP mock of the contract: frontend dev server and behavior reference for the backend |
| [`frontend/`](frontend/README.md) | Данил | React/TypeScript/TanStack application (FSD); its own rules are in [`frontend/AGENTS.md`](frontend/AGENTS.md) |
| [`infra/`](infra/README.md) | Данил | Demo stand on one DigitalOcean droplet: provisioning, deployment, nginx |
| [`docker-compose.yml`](docker-compose.yml), `*/Dockerfile` | shared | The platform in containers: the Rust API and web ([`infra/README.md`](infra/README.md#docker)) |
| [`docs/product/`](docs/product/BUSINESS_RULES.md) | shared | Business rules as implemented, requirements traceability, the organizer's specification |

## Rules

- Change another owner's tree only in coordination with them.
- The contract is the boundary between frontend and backend. Change it only as described in
  [`contracts/README.md`](contracts/README.md), in one change that updates every side.
- Work on a feature branch and PR it into your personal branch (`danil` or `dima`). Feature branches must not
  live under the `danil/` or `dima/` prefixes — those are the permanent personal branches, and git cannot hold
  both `danil` and `danil/x`. The external [CI controller](infra/ci/README.md) then merges verified personal → `dev` → `main` PRs.
  Delete the feature branch after merging; keep `danil`, `dima`, `dev` and `main` permanent. `main` is what
  `infra/deploy.sh` ships to the stand.
- Owner standard: do not create or schedule data/configuration backups, snapshots, off-host backup copies,
  or redundant repository mirrors unless the owner explicitly requests them. FSP backup units stay masked;
  provisioning and CI must not re-enable backup creation. Existing backup archives were deleted.
  Synchronize local branches with canonical `upstream` only; do not update the retired GitHub mirror.
- Never commit secrets. Stand access lives in the git-ignored `.fsp-infra/`; service secrets belong in environment
  files on the host.
- Write code, comments, documentation and commit messages in English; user-facing text is Russian.
- JavaScript-ecosystem code is strict TypeScript 7 checked by `tsc`; no hand-written JavaScript and no older
  TypeScript.

## Toolchains and checks

| Module | Toolchain | Check |
| --- | --- | --- |
| contract, mock, frontend | TypeScript 7.0.2 (strict) on Bun 1.4.2; Vitest on Node 26 (24.15 or newer works); Biome 2.5 from the root `biome.json` | `just verify`, `just e2e` |
| backend | Rust 1.99.0 (`backend/rust-toolchain.toml`), edition 2024 | `just backend-check` |

`just` lists the tasks for the TypeScript modules, the backend, Docker and the stand.
CI runs on the existing private server through [`infra/ci/`](infra/ci/README.md), reports four `fsp/*`
commit statuses, and deploys verified `main` with real public browser acceptance. GitHub Actions is retired.
