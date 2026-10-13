# ФСП / Карьера — frontend

The web application of the FSP reverse-hiring platform. A candidate confirms a specialization and grade with a
knowledge test, enriches the profile from sources and receives salary-bearing invitations; an employer
describes needs, searches an anonymous bank of test-confirmed and estimated candidates, keeps shortlists and invites a specific person,
whose contacts open only to that company after the candidate accepts. As an additional path, the employer can
publish a need as a vacancy and the candidate can apply to it, choosing which contacts the company receives; and
each week the platform offers the candidate a short task from a company, whose answer keeps the profile fresh.

The app talks to the API described in [`../contracts/openapi.json`](../contracts/openapi.json). Local development uses the
stateful [`mock-server`](../mock-server/README.md); the public stand uses Rust/PostgreSQL, real email and
source analysis. Seed accounts and the FSP provider are explicitly fictional. The domain rules the screens show are in
[`../docs/product/BUSINESS_RULES.md`](../docs/product/BUSINESS_RULES.md).

## Documentation

| Document | Contents |
| --- | --- |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Layers and slices, routing, data layer, cache scoping, mutations, privacy boundaries, build flags |
| [`docs/SCREENS.md`](docs/SCREENS.md) | Every route: what it shows, what it does, its states, with screenshots |
| [`docs/DESIGN_SYSTEM.md`](docs/DESIGN_SYSTEM.md) | Tokens, UI kit, status vocabulary, layout and accessibility |
| [`docs/TESTING.md`](docs/TESTING.md) | Unit, browser and stand tests, what each covers, how to run them |
| [`docs/DEMO_SCRIPT_RU.md`](docs/DEMO_SCRIPT_RU.md) | Демонстрационный сценарий для показа |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | Rules for changing the frontend |

## Stack

Bun 1.4.2 (package manager and runtime), React 19.3, TypeScript 7.0.2, Vite 8.3, TanStack Router 1.170 (file
routes), TanStack Query 5.104, TanStack Form 1.33, openapi-fetch 0.17 with types generated from the contract, zod 4.6
(URL search parameters), @xyflow/react 12.12 (competence graph, loaded lazily), Vitest 5 with Testing Library, jsdom
and MSW 3, Playwright 1.63 with axe, Biome 2.5. Exact versions are pinned in `package.json` and `bun.lock`.

Everything runs on Bun except Vitest, which runs on Node (26 in CI, 24.15 or newer works): its jsdom environment
fails on Bun's `EventTarget` and MSW cannot patch Bun's globals. `bun run test` starts it on Node.

TypeScript is strict (`exactOptionalPropertyTypes`, `noUncheckedIndexedAccess`). `skipLibCheck` is on because the
@xyflow/react declarations do not compile under `exactOptionalPropertyTypes`; project sources are checked in full.

## Run locally

```sh
bun install --frozen-lockfile
bun install --cwd ../mock-server --frozen-lockfile
bun run mock      # terminal 1: mock API on 127.0.0.1:4010
bun run dev       # terminal 2: http://127.0.0.1:5173
```

From the repository root, `just install`, `just mock` and `just dev` do the same.

Sign in with any address; a new address creates an account of the chosen role. Seed accounts: `alexey@example.com`
(candidate №024, verified Джуниор, target Мидл), `hr@sever-tech.example` («Север Тех»),
`hr@second-company.example` (a second company for privacy checks). The code is `123456`; in development
«Вставить код из демо-ящика» fills it in, and the «Демо» dialog switches actors, resets mock scenarios and moves the
mock clock. None of these controls exist in a production build.

To run the unchanged app against another API, for example a local backend:

```sh
FSP_API_TARGET=http://127.0.0.1:8080 bun run dev
```

## Scripts

| Script | What it does |
| --- | --- |
| `bun run dev` / `build` / `preview` | Vite dev server, production build into `dist/`, preview of the build |
| `bun run mock` | Starts the mock API |
| `bun run codegen` / `codegen:check` | Regenerates or checks `src/shared/api/generated/` from the contract |
| `bun run typecheck` | `tsc --noEmit` |
| `bun run lint` | Biome and the FSD layer check (`scripts/check-layers.ts`) |
| `bun run test` | Vitest unit and component tests |
| `bun run e2e` | Playwright against the mock (starts the mock and Vite if needed) |
| `bun run e2e:public` | Playwright smoke against a deployed stand (`FSP_PUBLIC_URL`) |
| `bun run verify` | codegen check, typecheck, lint, unit tests, build and bundle check (`scripts/check-bundle.ts`) |

## Configuration

| Variable | Read by | Effect |
| --- | --- | --- |
| `FSP_API_TARGET` | Vite dev and preview servers | Where `/api` (and in dev `/__mock__`) is proxied; default `http://127.0.0.1:4010` |
| `VITE_DEMO_STAND` | Build | `1` labels fictional data and displays the three disclosed seed accounts |
| `VITE_DEMO_EMAIL_CODE` | Build | Shown for `mock_mailbox` challenges; never used for arbitrary real-mail addresses |
| `VITE_API_ORIGIN` | Build, tests | Absolute API origin; empty (same origin) in the app, set by tests because Node cannot resolve relative URLs |

The stand builds with `VITE_DEMO_STAND=1` to identify fictional data. Arbitrary addresses use actual
Resend mail. Three exact seeded demo addresses retain a disclosed fixed code; the challenge delivery mode selects
the appropriate guidance. Demo sign-in does not prove mailbox ownership. Public acceptance creates
a candidate and two companies using real unique test inboxes; OTPs/cookies are excluded from traces/screenshots.
See [the stand runbook](../infra/README.md) for private acceptance credentials.

## Known limits

- Development uses the mock; the stand uses Rust/PostgreSQL with deterministic knowledge tests and actual
  isolated digital-PDF extraction/Bifrost analysis. FSP is a fictional HTTP provider. Email uses Resend; GitHub OAuth setup is deferred.
  Tests and configured AI integrations do not establish professional experience or authorship.
- The candidate's «Для работодателей» preview is assembled from the non-identifying fields of their own profile;
  the contract has no endpoint that returns a candidate's own anonymous projection.
- Every protected navigation revalidates the session with one `GET /api/v1/session`.
