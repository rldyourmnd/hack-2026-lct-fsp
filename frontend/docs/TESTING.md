# Testing

The [private CI controller](../../infra/ci/README.md) checks admitted canonical PRs and main.
`fsp/platform` covers TypeScript, the mock and frontend; `fsp/backend` covers real PostgreSQL and the exact
systemd PDF sandbox; `fsp/compose` covers containers; `fsp/release-safety` covers deployment activation rollback.
Backup creation and dump/restore checks are disabled by owner policy.
`fsp/deployment` passes only after actual HTTPS/Resend browser acceptance. GitHub Actions is retired.

| Suite | Runner | Where | Command |
| --- | --- | --- | --- |
| Mock API | Bun | `mock-server/test/mock-api.test.ts` | `bun run --cwd ../mock-server test` |
| Unit and component | Vitest + Testing Library + MSW (jsdom, on Node) | `src/**/*.test.ts(x)` | `bun run test` |
| Browser acceptance | Playwright + axe, Chromium | `e2e/*.spec.ts` | `bun run e2e` |
| Deployed stand | Playwright, real Rust/Resend/Bifrost | `e2e/public/stand.spec.ts` | `FSP_PUBLIC_URL=https://nddev.link FSP_AUTH_MODE=resend bun run e2e:public` |

Use the current test report for counts; `bun run e2e --list` lists collected browser scenarios. Documentation
screenshots are opt-in. `SCREENSHOTS=1 bun run e2e e2e/interview.spec.ts` refreshes the interview desktop/mobile
examples with fictional data.

`bun run verify` runs the codegen check, typecheck, Biome, the FSD layer check, the unit tests, the production build
and the bundle check; `just verify` adds the mock tests. JSON reports go to `test-results/`.

## Mock API tests

HTTP tests against an in-process server, one per rule group: contract drift (the committed `openapi.json` must equal
the generated one), the error-code catalog (every code, status and literal message the mock raises is listed in
`contracts/error-codes.json`), origin/host/role boundaries, the stand configuration (only the configured origin, no
development routes), e-mail verification and expiry, the server-owned category, automatic source jobs without
inferred hours or grades, the assessment lifecycle, failure and service-error outcomes, same-grade retry, downgrade
refusal and cooldown, graph-interview prerequisite traversal and stale revisions, stale checks and revoked consent,
need validation and tenant isolation, the anonymous and
deterministic bank, shortlist persistence, invitations with frozen terms, idempotency and per-company disclosure,
decline and withdrawal, vacancy publication and the candidate feed, applications with frozen terms, eligibility,
disclosure and its withdrawal, weekly task offers, answers, reviews and profile freshness, task authoring and
archiving, PDF export and import, and scenario resets. `test/contract-coverage.json` records which
operations and statuses were exercised.

## Unit and component tests

MSW intercepts requests at `http://localhost:3000` (`VITE_API_ORIGIN` in tests) and fails on any unhandled request.

| File | Covers |
| --- | --- |
| `shared/api/errors.test.ts` | Error envelope, non-JSON errors, missing bodies, human messages, field mapping |
| `pages/employer-need-editor/need-form.test.ts` | Whole-ruble salary parsing and rejection of malformed input, reversed ranges, requirement rules, the request payload |
| `entities/qualification/ui.test.tsx` | Verified grade never shown without verification; target kept apart |
| `features/shortlist-membership/use-membership.test.tsx` | Optimistic add rolled back on rejection; removal reflected everywhere |
| `features/respond-invitation/invitation-decision.test.tsx` | At least one disclosed field, exactly the chosen fields sent, conflicts keep contacts closed, no decision on a withdrawn invitation |
| `pages/login/login-page.test.tsx` | Address validation, wrong-code recovery, role conflict, real-mail guidance without demo hints |
| `pages/assessment-workspace/workspace-page.test.tsx` | Acknowledged and failed saves, serialized overlapping edits, submission prerequisites |
| `features/connect-source/use-source-processing.test.tsx` | Inline terminal jobs refresh derived data and expose failures |

## Browser acceptance tests

Playwright starts the mock and Vite when they are not running and talks to the real mock through the Vite proxy.
Tests run serially because they share the mock; each resets the scenario it needs through `/__mock__/reset` and
uses one browser context per actor, so a candidate and two companies act at the same time. Names carry the IDs of the
acceptance cases they cover.

| Spec | Cases |
| --- | --- |
| `auth.spec.ts` | Registration with a wrong and an expired code; role separation and deep links that return after sign-in; an expired session clears private data; logout |
| `candidate.spec.ts` | A new candidate reaches a verified category without FSP; a graph interview descends to a missing prerequisite and survives reload; the knowledge-test draft survives a reload, an upgrade updates every view and the PDF is real; a failed upgrade keeps Джуниор; a service failure is retryable without penalty; the cooldown comes from the server; automatic source enrichment and empty FSP history; a verified FSP record; the competence graph loads on demand and shares selection with the keyboard-accessible list; PDF import applies only on confirmation |
| `employer.spec.ts` | Salary validation, read-only tax basis, saving without publication; filters in the URL with a shortlist that survives filtering and reload; required coverage, verification gaps and verified grade kept apart; publication only on the needs list and no chat or ATS controls; a search failure is an error with retry; an emptied bank keeps shortlist entries as unavailable; graph loading/failure/manual retry is explicit and list mode makes no graph request |
| `invitations.spec.ts` | Anonymous until acceptance, then only the chosen fields and only for the inviting company; viewing changes only the status and declining grants nothing; a stale tab sees the canonical decision and a withdrawn offer cannot be accepted |
| `vacancies.spec.ts` | Publishing and unpublishing a need, the candidate feed with filters, salary and fit; applying opens only the chosen contacts to that company, which views and accepts while the candidate follows the status; withdrawing takes the contacts back; a candidate without a verified grade is told why applying is unavailable |
| `tasks.spec.ts` | The weekly task is answered once, reviewed by its company with a comment and makes the profile fresh for every company, while the answer stays with the author; companies add and archive tasks with validation; a candidate without a verified grade is told why there is no task |
| `accessibility.spec.ts` | No serious or critical axe violations (WCAG 2.2 AA tags) on the login, nine candidate and seven employer pages; skip link and keyboard navigation; narrow screens without horizontal scroll and with reachable navigation |
| `screenshots.spec.ts` | Opt-in (`SCREENSHOTS=1`): regenerates `docs/screenshots` |

Responses are also inspected: employer-side API bodies must not contain private markers (the seed candidate's name,
e-mail, repository names, references or fixture links) before disclosure.

## Deployed stand

`e2e/public/stand.spec.ts` checks HTTPS, deep links, capability flags, no-store and blocked mock controls.
Its second scenario creates a new candidate and two new companies, completes a knowledge test, starts and resumes a
persisted graph interview, exports a real PDF, accepts an invitation and verifies that the other company still cannot
see private contacts.
All actors use unique addresses; no seeded company login or production reset is used.

For nddev.link, set `FSP_AUTH_MODE=resend` and inject `FSP_RESEND_ACCEPTANCE_KEY` privately. The helper
[`e2e/public/mail.ts`](../e2e/public/mail.ts) reads only dedicated `fsp-ci-*` addresses at
`FSP_TEST_EMAIL_DOMAIN` (default `auth.nddev.link`), requires the expected sender and passing DKIM/DMARC,
and waits up to two minutes for actual mail. Missing credentials or delivery failures fail acceptance.
Public traces/screenshots are disabled to avoid retaining OTPs/cookies. Test records persist in PostgreSQL.
The root-managed CI configuration and safe invocation are in the [stand runbook](../../infra/README.md#domain-and-resend).
Local demo acceptance can explicitly use `FSP_AUTH_MODE=demo` and `FSP_DEMO_CODE` (default `123456`).
Never use demo acceptance configuration for nddev.link.

The live exploratory pass on 2026-10-08 used Chromium against the HTTPS Rust/PostgreSQL stand with separate
candidate and employer contexts, no interception and no mock controls. It covered registration, saved drafts,
assessment submission, PDF download/import, real graphs, company/need forms, persistent shortlists, selective
invitation disclosure, cross-company isolation, applications/withdrawal, weekly task review and mobile navigation.
It found backend-specific PDF wording and cramped mobile card headings; the fixes and final deployed screenshots
are tracked in [issue #23](https://github.com/KirpaDmitriy/sport-programming-federation/issues/23).
That earlier pass tested the then-simulated assessment and import. Follow-up [issue #31](https://github.com/KirpaDmitriy/sport-programming-federation/issues/31)
replaces them with deterministic knowledge tests and actual sandboxed digital-PDF extraction/Bifrost fields.
Local real-backend browser evidence covers passing/failing tests, draft restoration, cited PDF confirmation,
contextual assistance, FSP HTTP published/pending/empty results, and preserved snapshots during provider outage.
That issue records the earlier product release. Real Resend delivery and owner login subsequently passed in
[#38](https://github.com/KirpaDmitriy/sport-programming-federation/issues/38), with private CI in
[#40](https://github.com/KirpaDmitriy/sport-programming-federation/issues/40). Only GitHub OAuth remains deferred;
FSP remains fictional. Current checks and limitations are summarized in [current state](../../docs/current-state.md).

## What the tests do not show

Mock suites prove contract shapes, state transitions, access rules and the interface against fixtures. The
separate live suite proves actual Rust/PostgreSQL flows and authenticated mail delivery. Neither establishes
assessment validity, matching quality, high-load capacity or a comprehensive production security review.

The live browser pass also exposed an outer API timeout shorter than the gateway budget and hidden employer
graph failures. [Issue #27](https://github.com/KirpaDmitriy/sport-programming-federation/issues/27) tracks the
55-second API deadline, on-demand graph queries and manual retry. Its forced-error browser regression runs
locally against an intercepted response; this is separate from the unmodified HTTPS stand evidence in #23.

## Failure-safe public acceptance

`e2e/public/actors.ts` owns only the fresh `fsp-ci-*` actors registered by the public suite. Its fixture has a
separate 90-second teardown budget: unpublish the candidate, remove all needs owned by the newly created
companies, then close every owned browser context. Teardown runs after test failure too; cleanup failures fail
the release check. Accounts, private interview history and invitations persist; no database reset or unrelated
actor cleanup occurs. Process termination or infrastructure failure can still prevent cleanup and requires
inspection of the failed run. `e2e/acceptance-cleanup.spec.ts` injects failure after writes and verifies cleanup,
identity guarding, context closure and preservation of another company's need against the local mock.

`e2e/interview.spec.ts` uses a controlled browser speech adapter to exercise startup errors and late callbacks.
It proves application behavior, not physical microphone or vendor speech availability. Public acceptance checks
the actual delivered Permissions-Policy and completes a text answer against the real model.
