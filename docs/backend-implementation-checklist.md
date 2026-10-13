# Backend implementation status

Current implementation; [current system state](current-state.md) describes verification paths and known limits.
Historical cutover commands for another contributor's server are not operating instructions for this stand.

## Implemented

- Rust hexagonal architecture: domain/application ports, axum transport, PostgreSQL and outbound adapters.
- All 79 application operations plus `/health`; no production `/__mock__` controls.
- Persisted accounts, sessions, profiles, consents, needs, vacancies, invitations, applications, shortlists,
  tasks, assessments, graph interviews, sources and jobs. Roles and tenant boundaries are enforced by the API.
- Atomic idempotent commands and response replay; shared email attempt budgets; guarded terminal decisions;
  serialized disclosure and shortlist mutations; profile/assessment/source concurrency protection.
- Candidate visibility, deterministic ranking, filters, server-returned pagination and anonymous projections.
- GitHub connection by login through the tokenless public-only insights service as a polled background job,
  ownership by bio code or OAuth identity, and evidence updates from substantively developed repositories that
  preserve verified grades.
- Automatic profile (`estimate-v6`, with a growth plan, focus skills and the review's advice; an outdated
  estimate refreshes itself when the profile opens): every source write recomputes the estimate in its transaction, answers the
  questionnaire fields still automatic (soft skills included) and gives an estimated category; editing stated
  experience or team role recomputes it; a bounded Bifrost second opinion with a level rubric; manual recalculation
  that re-analyzes outdated GitHub reports and can rebuild every answer; category source and confidence in search,
  ranking, applications and weekly tasks.
- Deterministic competence graph: category → concepts → technologies with levels, GitHub/FSP/test sources, safe
  employer projection.
- Graph-guided candidate interviews through an internal Rust service: authenticated platform routes, PostgreSQL
  sessions with optimistic revisions, an offline CPU rubric, optional Bifrost generation/evaluation with one model budget, consent checks on start/answer and
  browser speech with text fallback. Browser speech can use a vendor service.
- Real PDF export/import, revision-checked preview confirmation, contextual Bifrost assistance, source/job privacy and capability flags.
- Bounded Resend delivery, per-email issuance limits, cumulative wrong-code budgets and challenge/session HMAC separation.
- Rustfmt, clippy, unit tests and PostgreSQL HTTP/storage regressions; private-server CI gates cover admitted canonical PRs.

## Explicit demo limits

- Public authentication uses actual Resend delivery, random single-use codes and keyed digests for arbitrary
  addresses. Three exact seeded demo addresses retain the disclosed fixed code; demo verification is not mailbox
  ownership proof.
- FSP uses a separate fictional HTTP provider with bounded validation, published/pending/disqualified results
  and durable snapshots. No official API, identity verification or dataset is implied.
- Knowledge tests use a versioned server-side bank and deterministic scoring. The public study bank is not
  leak-resistant or professionally calibrated. Legacy simulated code attempts remain archived; no code executes.
- Contextual assistant replies call Bifrost and never decide grades.
- Interview evidence is an adaptive rubric signal, not a calibrated hiring score. The shipped graph covers one
  backend-middle blueprint; video, camera analysis and biometric or inferred-personality scoring are intentionally absent.
- PDF import extracts digital text in a bounded Poppler sandbox; scans/OCR are unsupported. Bifrost fields require actual page excerpts and user confirmation.
- FSP/PDF jobs complete inline; GitHub jobs advance by polling a process-local analysis queue. Estimate reviews
  run in coalesced background tasks. There is no durable background worker queue.
- Repository/competition matching has an adapter port but no product workflow. Competence graphs are deterministic projections
  of stored evidence, not independently verified qualifications.
- Search still ranks complete lists in memory. No distributed scaling or measured capacity claim is made.

## Delivery evidence

The public stand is `https://nddev.link` (origin `188.166.88.183`), managed by [`infra/`](../infra/README.md). Main is deployed through
feature → personal → dev → main with successful checks. Release activation rollback has automated probes. Backup creation is disabled by owner policy. Public browser acceptance and exact stand/main SHA verification use the
[CI statuses and deployment receipt](../infra/ci/README.md); local tests alone do not imply successful deployment.

Code and current verification results take precedence over a checked historical task.
