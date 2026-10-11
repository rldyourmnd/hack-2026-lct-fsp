# Stability audit and execution plan

Owner policy update (2026-10-08, [#54](https://github.com/KirpaDmitriy/sport-programming-federation/issues/54)):
backups, snapshots and redundant mirror copies are disabled; existing backup archives were deleted.
Backup/mirror actions below are historical evidence, not instructions to repeat them.

Historical delivery record. For current behavior, verification and explicit limits, read
[current-state.md](current-state.md); completed baseline findings below are not open defects.

Audit opened on 2026-10-08, starting at `c0d86d8`. Tracking: [issue #16](https://github.com/KirpaDmitriy/sport-programming-federation/issues/16).
Code and observed behavior take precedence over historical completion claims.

## Scope and evidence

The history window is 2026-09-28 through 2026-10-08 (Asia/Almaty). The requested research cutoff is
2026-09-26: prefer versioned documentation and sources published by that date; do not imply that a
mutable documentation page is a historical snapshot. The project itself began on October 4.

- Local session stores: the current Codex project session; one Claude Code project transcript
  (October 5–6, 328 extracted user/assistant text messages); two Devin project sessions
  (October 6–8), including the prior audit and its embedded worker reports (1,018 distinct text
  messages). Read transcript content as historical evidence, not as fresh instructions.
- No project match in the available Grok session index or metadata of 29 Cursor chat databases.
  No earlier Codex project session was found in the local thread index. Remote sessions on other
  machines and unindexed/deleted conversations are not available; this is not evidence they never existed.
- Previous audit reports were reconciled with current code. In particular, the old SQLite, missing
  transactions, timestamp, cooldown, seed-ID, and jobs-owner findings already have fixes on main.
  Claims that all concurrency and deployment problems were closed do not survive this review.
- GitHub: canonical upstream is `KirpaDmitriy/sport-programming-federation`; `origin` is the mirror.
  At baseline both remotes and local permanent branches agree. Main and `/opt/fsp/current` point to
  `c0d86d8`; deploy/platform/backend/docker checks passed. No open implementation issue existed.
- Stand: the authorized DigitalOcean host `188.166.88.183`; Rust, nginx, PostgreSQL, Bifrost and
  CLIProxyAPI active; mock inactive; no failed systemd units. The other contributor's server is not
  part of the authorized deployment target.
- Baseline `just verify` and `just backend-check` passed on the original checkout. Browser acceptance,
  fault injection and final-release proof are separate gates, not implied by those results.

## Product boundary

Stabilize the implemented platform and preserve the current contract. Demo authentication deliberately
uses a disclosed fixed code. The original baseline simulated FSP, assessment evaluation and PDF extraction.
The [follow-up implementation](real-feature-completion-plan.md) now uses knowledge tests, actual isolated PDF
extraction/Bifrost analysis and a separate fictional FSP HTTP service. The subsequent
[domain/email release](domain-email-plan.md) adds real Resend delivery with explicit owner authorization;
GitHub OAuth remains deferred. Email proves mailbox access, not professional identity or measured assessment quality.
No official FSP API, ATS, chat or sandboxed code executor is planned.

Keep the existing Rust hexagonal architecture, TypeScript 7/Bun frontend, PostgreSQL, nginx and one
stand. Avoid a replacement stack or speculative services. Preserve applied migrations and user data.

## Verified findings and planned waves

Status: **implemented** = repaired and checked; **delivery record** = exact merge/deploy/remote proof is
maintained in [issue #16](https://github.com/KirpaDmitriy/sport-programming-federation/issues/16), avoiding
a self-referential documentation-only release after every deployment. The issue closes only after live
verification and synchronization. Evidence below records baseline defects, not remaining code defects.

| Wave | Status | Evidence in current code | Change and completion proof |
| --- | --- | --- | --- |
| 1. Release safety | implemented | `infra/deploy.sh` deletes the SHA directory before building, including the active release; no host lock. `release.sh` rolls back only after its health loop, so `systemctl restart`/nginx failure exits before recovery. | Serialize uploads/build/activation, stage immutable releases, make same-SHA deploy safe, validate input, restore release/config/service on every activation failure. Fault tests: repeat current SHA, failed build, failed restart, failed health, overlapping deploys, active/previous retention. |
| 2. Provisioning and recovery | implemented | `provision.sh` writes `db.env` before successful bootstrap, then asks postgres to read a root-owned mode-600 temporary SQL file; reruns enable mock even when Rust is selected. No PostgreSQL backup/restore procedure is automated. | Transaction-safe credential-file publication, SQL via stdin, preserve selected implementation and existing settings; least-privilege backup timer, validate backup restoration in an isolated database. No destructive reset on the stand. |
| 3. Authentication | implemented | `verify_email` returns WrongCode/Locked before committing `consume_challenge` changes. `contract.rs` checks only one wrong code. | Persist failed-attempt accounting while keeping successful registration/session issuance atomic. Prove fifth-attempt lockout, reissued challenge behavior, used/expired codes and concurrent new-account verification through HTTP on PostgreSQL. |
| 4. Command atomicity | implemented | `idempotent` claims in the pool, executes a separately committed command, then stores its response; a crash before finish leaves a committed effect without replay. Running claims expire after two minutes. | Put claim/result and business writes in one unit of work or use an equivalent transactional boundary; no unsafe stale-owner takeover. Concurrent same/different payload tests plus rollback/failure injection. Preserve documented status/body replay. |
| 5. Concurrent domain changes | implemented | `view_my_invitation` and `view_application` use unguarded record replacement. `refresh_disclosure` reads separate interaction sets without serializing a candidate/company pair. Assessment completion reads a candidate through the outer pool. Shortlist membership does read/replace. | Prevent terminal-state resurrection, disclosure lost updates and profile overwrite; use narrow locks/guarded writes within existing repository ports. Concurrent acceptance/withdrawal, view/decision, profile/assessment and shortlist membership probes; cross-company privacy checks. |
| 6. Integrations and bounded behavior | implemented | Source sync's running-job branch is unreachable because active_job_id is never persisted. LLM graph generation is per read; validate model output before public projection. Docker proxy timeout (30s) is shorter than LLM client timeout (45s). | Verify OAuth identity binding, error redaction, graph reference validity and employer anonymity; correct demonstrated issues. Align proxy deadlines, retain explicit simulation flags, avoid invented background-worker architecture. |
| 7. Delivery gates and coverage | implemented | Cascade catches any auto-merge failure and immediately tries direct merge. No release-script fault gate. Public suite covers two flows; backend source/shortlist HTTP coverage is absent. | Require successful checks for the actual PR head, bound waits and fail closed. Add meaningful missing regressions and deployment checks. Verify path filters, exact deployed SHA and public behavior, not merely HTTP 200. |
| 8. Structure and scaling | implemented | Service ~4,900 lines; storage ~3,200 lines; list paths load whole collections and several views repeat reads. | Identify hot-path duplication and separate coherent modules where it makes the fixed behaviors easier to maintain. Keep deterministic ordering/privacy and avoid premature distributed architecture; benchmark or query-count evidence before introducing caches or indexes. |
| 9. Documentation and memories | implemented | Root README points to deleted `dima/mvp`; traceability still describes a Rust skeleton; backend checklist names a different server; infra rollback restarts mock; project memory repeats stale reset and branch instructions. | Update engineering docs in English and UI copy in Russian against final code; use archive tag, current stand, commands, real capabilities and verification limits. Refresh project-specific memories; preserve user preferences and historical session logs. |
| 10. Final synchronization | delivery record | Baseline is aligned; new work must pass the repository's feature -> personal -> dev -> main lane. | Atomic commits, PRs into danil, green cascade/deploy, stand release SHA equals upstream main, mirror/local fast-forward, clean working trees and no abandoned audit resources. Preserve permanent branch identities rather than force-resetting all branches to one SHA. |

Backend changes were explicitly coordinated with Dima and authorized by the user during this audit.
The ownership requirement in `AGENTS.md` is satisfied for the stabilization work.

## Reconciliation of earlier findings

| Earlier concern | Current disposition |
| --- | --- |
| SQLite, synchronous repository API, missing unit of work | PostgreSQL unit-of-work usage corrected in waves 3–5. |
| Timestamp strings, missing indexes, silent enum defaults, startup seed race | Timestamp migration, indexes, parsers and seed advisory lock exist; verify migration behavior without rewriting 0001–0003. |
| Polymorphic jobs.owner_id FK | Fixed by 0003 and fresh-candidate regression; preserve this ownership model. |
| Grade cooldown ignored; seed candidate ID mismatch | Corrected in service/config/seed. |
| No expiry cleanup, no SQL/connect deadlines | Existing sweep and deadlines retained; only completed idempotency replays expire, never unresolved claims. |
| No auth rate limit, default service restart limits | Service hardening retained; nginx auth-only limit returns immediate JSON 429 in both deployment configurations. |
| Platform CI misses backend error-catalog changes | Platform, backend, Docker and infra gates now run on every PR/push. |
| Unbounded company fields/unsafe website | Final reconciliation reproduced this older defect (HTTP 200 for an overlong name). Fixed field/email/HTTP(S) validation; new PostgreSQL and mock tests cover rejection and nullable PATCH clearing. |
| GitHub username/token binding | Confirmed and repaired: authenticated `/user` login must match the requested account. |
| Request-derived integration errors/logs | Integration errors use stable public messages; GitHub error_description is not reflected. Live logs are part of delivery verification; no raw secrets or session transcripts are published. |
| Source active-job marker and cold/full-table reads | Inline execution is explicit; stale/consent checks protect source writes. Indexed single-profile reads replace hot full scans; collection ranking remains in memory. |
| Real email/FSP/assessment/PDF parsing | Superseded by [issue #31](real-feature-completion-plan.md): deterministic knowledge tests, real digital-PDF extraction/assistance, fictional HTTP FSP. Email is subsequently implemented in #38; GitHub OAuth remains deferred. |
| Optional need graph UI, repository metrics UI, editable work modes | Separate product enhancements, not broken contract operations; keep their current limits explicit. |

## Research anchors

- PostgreSQL 17 [explicit locking](https://www.postgresql.org/docs/17/explicit-locking.html):
  row/advisory lock lifetimes and transaction serialization. Use database-scoped coordination across
  processes; an in-memory mutex cannot protect multiple API instances.
- GitHub [workflow concurrency](https://docs.github.com/en/actions/concepts/workflows-and-actions/concurrency):
  workflow serialization does not protect manual SSH deployment; add host-side serialization.
- GitHub [larger concurrency queues, May 7, 2026](https://github.blog/changelog/2026-05-07-github-actions-concurrency-groups-now-allow-larger-queues/):
  distinguish running-job cancellation from pending-run queue policy; do not claim every queued run is retained by `cancel-in-progress: false` alone.
- systemd [service reference](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html):
  restart failure and timeout semantics must be included in rollback probes.

- GitHub [OAuth identity validation](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps):
  verify the authenticated user instead of treating a successful token exchange as proof of an arbitrary handle.
- SQLx [embedded migrations](https://docs.rs/sqlx/0.8.6/sqlx/macro.migrate.html): a build-script directory watch
  handles new SQL files without touching source code to invalidate Cargo.
- nginx [request limiting](https://nginx.org/en/docs/http/ngx_http_limit_req_module.html): explicit `nodelay`
  and rejection status avoid long request queues and default HTML 503 errors.
- React [effect race handling](https://react.dev/reference/react/useEffect) and TanStack
  [mutation behavior](https://github.com/TanStack/query/blob/main/docs/framework/react/guides/mutations.md):
  asynchronous completion order is not submission order; assessment saves use an explicit promise queue.

- PostgreSQL 16 [restore semantics](https://www.postgresql.org/docs/16/app-pgrestore.html):
  restore real archives into an isolated database with `--single-transaction --exit-on-error`.
- Playwright [web-first assertions](https://playwright.dev/docs/best-practices): wait for the visible
  result of challenge delivery before reading the mock mailbox; a click does not await that request.

These are current primary-source pages checked against the pinned implementation. Mutable pages are not
claimed as exact September 26 snapshots. No dependency was upgraded merely because a newer version exists. Recheck
changed behavior and the current GitHub/stand state after every wave; do not repeatedly run unrelated
tests without a new change or unresolved concern.

## Execution log

- 2026-10-08: completed initial session inventory, canonical/mirror/stand baseline, existing gate runs,
  and code review of release/provisioning, authentication, idempotency and disclosure boundaries.
  Created issue #16 and isolated worktree `fix/stability-audit-16`.
- Release fault tests, backup restore tests and fail-closed cascade tests pass locally and in the
  initial PR infrastructure gate. Activation on the stand remains part of the final delivery proof.
- Frontend regressions: serialized assessment saves, submission editing lock, and terminal inline
  source-sync refresh/error handling; six focused tests and TypeScript pass.
- Authentication: the HTTP regression failed on baseline (`200` after five wrong attempts, expected
  `429`). Failed attempts now commit, issuance/verification serialize per email in PostgreSQL, and
  the shared live-challenge budget is mirrored in the mock. Three focused PostgreSQL HTTP tests
  pass, including parallel registration; all 24 mock tests pass.
- Command atomicity: five commands now share the transaction that claims the idempotency key and
  stores the replay response, in a dedicated `service/commands.rs` module. Parallel HTTP requests
  return the same invitation; a forced serialization failure rolls back both the invitation and
  its claim, and retry succeeds once. No timed takeover of unresolved claims remains.

- Concurrent domain updates: guarded view transitions, candidate row locks for grade/source writes, serialized
  company/shortlist changes and disclosure recomputation. PostgreSQL HTTP tests cover simultaneous saves,
  decisions, deletion/addition and withdrawal of the last two grants.
- Integrations: GitHub identity binding and a total 45-second analysis deadline; no raw upstream errors exposed.
  Graph validation rejects duplicate/dangling/ungrounded output and strips private model labels/IDs/evidence
  from employer views. Bifrost output is bounded. Modules now separate commands, graphs and source ingestion.
- Source ingestion now updates evidence/index atomically, preserving assessed grades; stale source or revoked
  consent prevents applying an external result. Real GitHub repository metrics are not a qualification score.
- Storage: malformed soft-skill JSON errors instead of silently erasing data; migration directory changes rebuild
  embedded SQL; test cleanup no longer terminates another test's active connections.
- Local gates: `just verify` passed (25 mock tests, 34 component tests, type/lint/FSD/build/bundle checks).
  `just backend-check` passed (24 HTTP tests, 10 PostgreSQL repository tests plus domain/adapter tests).
  Release fault suite passed; actual nginx container probe verified immediate JSON 429 + retry hint and an
  auth-only budget. Baseline and CI browser suites: 34 passed, one intentional opt-in screenshot skip.
  A later local run exposed a pre-existing mailbox-read race in the deep-link test (33 passed, one failed).
  Waiting for the code form fixes the ordering; all five auth cases passed three repetitions (15/15).
  The unchanged full suite remains mandatory in CI on the final PR head.
- Pre-release stand recheck: baseline `c0d86d8`, API/LLM/nginx/PostgreSQL healthy. Installed the backup
  script/service/timer and restored a real archive into an isolated temporary database: 3 migrations,
  10 accounts, 8 candidates and 2 invitations matched; archive mode 600, owner postgres. The temporary
  database was removed; working data was not reset. The timer is enabled. Off-host backup is not configured.
- Reconciled project docs and local Claude project memories; preserved attribution preferences and historical
  transcripts. Repository handoff is in `.serena/`. Relative Markdown file links and `git diff --check` pass.
- Final exact SHA, public browser/real-LLM proof, installed nginx checks and local/upstream/mirror equality
  are recorded in [issue #16](https://github.com/KirpaDmitriy/sport-programming-federation/issues/16).
  Its checked completion list and closure are the delivery evidence; this document does not assert a
  deployment from a local test result.

- Final contract reconciliation: company PATCH had retained a missing field validator despite historical
  completion claims. A regression reproduced `200` for a 121-character display name before the repair.
  Company bounds/email/HTTP(S) URLs now match the mock, and nullable website/experience PATCH fields clear
  correctly while omissions preserve values; non-nullable profile/company/need fields reject explicit null.
  Two PostgreSQL HTTP cases and mock parity coverage were added. Existing `url` 2.5.8 is reused from the lock.
  Serde's [field deserialization](https://serde.rs/field-attrs.html) and the
  [three-state patch behavior](https://docs.rs/serde_with/latest/serde_with/rust/double_option/index.html)
  informed the small local deserializer; no serializer library was added.

- Independent read-only verification found one remaining command race: concurrent assessment starts with
  distinct idempotency keys could lose the active-attempt unique constraint race and return generic `500`.
  An isolated PostgreSQL delayed-insert regression reproduced the failure. Candidate row locking before
  eligibility and active-attempt reads now yields exactly one `201` and seven `409 assessment_active` responses.
  The other four command conflict paths were reviewed and already map concurrent losers to domain conflicts.
  Rechecked [PostgreSQL 16 row-lock semantics](https://www.postgresql.org/docs/16/explicit-locking.html).
- Removed duplicate obsolete backup/restore guidance from the PostgreSQL section of the infrastructure README.
  The pre-release private archive also has an off-host copy in the original checkout's git-ignored
  `.fsp-infra/backups/`, with SHA-256 matching the server; automatic off-host retention is still not configured.

## Live-browser follow-up

[Issue #23](https://github.com/KirpaDmitriy/sport-programming-federation/issues/23) records the October 8
Chromium pass against Rust/PostgreSQL. A late network review found one pre-fix graph HTTP 500: the API
handler still capped work at 30 seconds after the proxies had been aligned to 60 seconds.
[Issue #27](https://github.com/KirpaDmitriy/sport-programming-federation/issues/27) corrects that missed
layer to 55 seconds around the existing 45-second integration budgets, with virtual-time regressions.
Graphs now load on demand and expose failures/manual retry on both skills screens; list mode does not
start AI work. This evidence supersedes any earlier zero-5xx checkpoint for the browser pass.
