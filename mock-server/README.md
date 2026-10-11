# Mock API

A stateful HTTP server that implements the contract in [`../contracts/openapi.json`](../contracts/openapi.json)
with synthetic data. It serves the frontend in development and as an explicit fallback, and it is the executable reference
of the business rules in [`../docs/product/BUSINESS_RULES.md`](../docs/product/BUSINESS_RULES.md) for the backend.
It is not production authentication, does not execute code, call models, read uploaded files or contact GitHub, FSP
or a mail server.

## Run

Written in strict TypeScript 7 and run by Bun 1.4.2 or newer; dependencies are pinned in `package.json` and `bun.lock`.

```sh
bun install --frozen-lockfile
bun start                       # http://127.0.0.1:4010
curl http://127.0.0.1:4010/health
# {"status":"ok","service":"fsp-mock-api","version":"0.1.0"}
```

| Variable | Default | Meaning |
| --- | --- | --- |
| `HOST` | `127.0.0.1` | Loopback only (`127.0.0.1`, `::1`, `localhost`) |
| `PORT` | `4010` | HTTP port |
| `FSP_MOCK_SCENARIO` | `baseline` | Initial scenario |
| `FSP_GRADE_CHANGE_COOLDOWN_DAYS` | `90` | Cooldown after a successful grade change, integer 0–365 |
| `FSP_MOCK_ALLOWED_ORIGINS` | `http://localhost:5173,http://127.0.0.1:5173` | Comma-separated browser origins accepted on requests with `Origin` |
| `FSP_MOCK_DEV_ROUTES` | `1` | `0` answers every `/__mock__` route with `404` |

State lives in memory: a restart or reset drops mock sessions, codes and jobs. The stand currently runs Rust
with PostgreSQL; restarting the dormant mock does not reset stand data. The optional mock systemd unit uses
`FSP_MOCK_DEV_ROUTES=0` and the stand origin ([`../infra/README.md`](../infra/README.md)).

## Seed data

The clock starts at `2026-10-05T12:00:00.000Z` and moves only through `/__mock__/clock`. The e-mail code is always
`123456`. All names, companies, repositories (`example.com/fixtures/...`) and competitions are synthetic.

| Actor | Role | Identity |
| --- | --- | --- |
| `candidate024` | candidate | Алексей Морозов, `alexey@example.com`, «Кандидат №024»: verified Бэкенд · Джуниор since 2026-06-01, target Мидл, GitHub connected (12 repositories, 3 queued at start), one passed assessment in the history |
| `candidate-new` | candidate | `new@example.com`, empty unverified profile |
| `employer-a` | employer | «Север Тех», `hr@sever-tech.example`, complete company |
| `employer-b` | employer | «Вторая компания», `hr@second-company.example`, complete company |

The bank also holds №018 (verified Бэкенд · Мидл, PostgreSQL without evidence, FSP connected without history), №031
(verified Бэкенд · Мидл, verified FSP participation) and №041 (verified Фронтенд · Мидл). Each company has a need
«Бэкенд-разработчик» (Мидл, remote, 200 000–260 000 ₽ gross per month; required Rust and PostgreSQL, desired Docker
and Redis); «Север Тех» keeps it unpublished for direct invitations, «Вторая компания» publishes it. Published
vacancies also include «Бэкенд-разработчик сервиса уведомлений» of «Север Тех» (Джуниор, hybrid, 120 000–160 000 ₽;
required Rust) and «Фронтенд-разработчик (React)» of «Вторая компания» (Мидл, office, 180 000–230 000 ₽; required
React). «Север Тех» has the shortlist «Бэкенд · Мидл» with №024, №018 and №041. Short tasks: «Север Тех» offers
«Повтор платежа без двойного списания» (PostgreSQL, Джуниор) and «Повторные попытки отправки уведомлений» (Rust,
Джуниор), «Вторая компания» «Форма регистрации с проверкой полей» (React, Мидл); №031 and №041 have accepted answers,
so their profiles are fresh, while the others' grade confirmations are older than 90 days. No invitations or
applications are seeded.

Any new e-mail registers a new account of the chosen role. The FSP IDs `FSP-DEMO-024` and `FSP-DEMO-031` (a longer fictional history) connect
with verified participation; any other ID connects without history.

## Scenarios

| Scenario | Effect |
| --- | --- |
| `baseline` | The seed data above; №024 can start a Мидл attempt |
| `qualified` | №024 is already a verified Мидл (confirmed at the seed clock, so a new grade change is in cooldown) |
| `assessment_failure` | Checks fail and submitted attempts end `failed`; grades stay |
| `service_failure` | Submitted attempts end `service_error`; no rubric, no cooldown |
| `no_fsp` | No candidate has an FSP connection |
| `empty_bank` | Search and profile lookups find nobody; shortlist members show as unavailable |
| `api_error` | Candidate search answers `503 fixture_search_unavailable` |
| `new_candidate` | The seed data; used with the `candidate-new` actor |

## Development routes

Available only from loopback and only while `FSP_MOCK_DEV_ROUTES` is not `0`; the frontend calls them from the
dev-only controls in the «Демо» dialog and the login page.

| Route | Purpose |
| --- | --- |
| `POST /__mock__/session` `{ actor }` | Sign in as a seed actor |
| `POST /__mock__/reset` `{ scenario }` | Reset all state to a scenario |
| `GET /__mock__/email-codes` | Read the demo mailbox |
| `POST /__mock__/clock` `{ advanceSeconds }` | Move the clock forward |

```sh
curl -c jar -H 'Content-Type: application/json' -d '{"actor":"candidate024"}' http://127.0.0.1:4010/__mock__/session
curl -b jar http://127.0.0.1:4010/api/v1/me/profile
```

## Boundaries

- Requests are validated with Ajv 2020 against the contract schemas; successful responses are validated too.
  Cross-field rules, access and state transitions are in `src/domain.ts`.
- Only loopback `Host` headers are accepted (`403 host_not_allowed`); behind nginx the stand rewrites `Host`.
- Jobs are timers: running after about 300 ms, finished after about one second.
- Assessment outcomes, checks and the assistant answer come from the scenario. The PDF import returns a fixed
  preview (`parser: fixture_preview`); the PDF export is a real document rendered with PDFKit and the bundled DejaVu
  Sans font (license in `fixtures/`).

## Files

| File | Role |
| --- | --- |
| `src/generated/schema.d.ts` | Contract types written by `../contracts/codegen`; the handlers' inputs and outputs are typed by them |
| `src/server.ts` | HTTP boundary: routing, session cookie, origin and host checks, body parsing, idempotency, error envelope |
| `src/domain.ts` | Business rules, projections, jobs; one handler per operation, checked for completeness and response shape by the compiler |
| `src/data.ts` | Seed data, task, rubric, catalogs |
| `src/contract.ts` | Route table and schemas; builds `contracts/openapi.json` |
| `src/validation.ts` | Ajv validators compiled from the schemas |
| `src/pdf.ts` | PDF export |
| `scripts/build-contract.ts` | Writes `../contracts/openapi.json` (`bun run contract`) |
| `test/mock-api.test.ts` | HTTP tests; `test/contract-coverage.json` records the operations and statuses they hit |

## Tests

```sh
bun run typecheck                  # tsc 7 over src, scripts and test
bun run lint                       # Biome with the repository configuration
bun run test                       # HTTP behavior tests against an in-process server
UPDATE_FIXTURES=1 bun run test     # also rewrites fixtures/profile-export.pdf
bun run contract                   # rebuild ../contracts/openapi.json after editing src/contract.ts
```

The tests cover the contract drift, origin/host/role boundaries, e-mail verification, profile category, source
jobs, the assessment lifecycle and its failure modes, cooldown, stale checks and revoked consent, need validation,
the anonymous bank and its order, shortlists, invitations with disclosure and terminal states, vacancy publication
and applications with disclosure and withdrawal, weekly tasks with reviews and freshness, the error-code catalog, PDF export and import, and scenario resets. A contract change follows [`../contracts/README.md`](../contracts/README.md).
