# API integration

How the API behaves on the wire and how the frontend uses it. Shapes, enums and status codes are in
[`openapi.json`](openapi.json); the operation list is in [`API_OPERATIONS.md`](API_OPERATIONS.md); domain rules are
in [`../docs/product/BUSINESS_RULES.md`](../docs/product/BUSINESS_RULES.md). Everything below is what the mock
implements and the frontend relies on; the backend has to keep it.

## Connection

- The browser calls the same origin. Application paths already start with `/api/v1`, so the client's base URL is
  empty (`VITE_API_ORIGIN` sets an absolute origin only in Node tests). nginx forwards only `/api/v1/`.
- The Vite dev and preview servers proxy `/api` to `FSP_API_TARGET` (default `http://127.0.0.1:4010`, the mock); the
  dev server also proxies `/__mock__`.
- `/__mock__/*` are development controls marked `x-mock-only` (choose actor, reset scenario, read the demo mailbox,
  advance the clock). They are not part of the application API. Rust never exposes them and nginx blocks them on the stand;
  `FSP_MOCK_DEV_ROUTES=0` additionally disables them in a standalone mock deployment.

## Session

1. `POST /api/v1/auth/email/challenges` with `{ email, role }` → `201 { challengeId, expiresAt, delivery }`
   (`delivery: "mock_mailbox"` in the mock, `"email"` for real Resend delivery).
2. `POST /api/v1/auth/email/verify` with `{ challengeId, code }` → `200 Session` and the session cookie.
3. `GET /api/v1/session` → `Session` (`actorId`, `role`, `emailVerified`, `candidateId`, `companyId`,
   `displayName`) or `401`.
4. `DELETE /api/v1/auth/session` → `204`, the cookie is cleared.

`meta.capabilities.realEmail` reports configured real delivery, not provider health.
The real-mail adapter generates random codes, stores challenge-bound keyed digests
and sends Russian text/HTML mail. In real-mail mode, issuing requests are serialized
per normalized email: one/minute and five/15 minutes, including failed sends.
Excess sends return `429 rate_limited`; provider rejection returns
`502 verification_delivery_failed`. Demo/local mailboxes keep their deterministic
behavior. Switching the production auth key invalidates prior codes and sessions.

Five incorrect codes exhaust the email's shared budget across all unused, unexpired challenges.
The fifth incorrect code returns `400 invalid_code`; later verification and issuance return
`429 challenge_locked` until the relevant challenges expire. Reissuing a code does not reset the
budget. Wrong attempts are persisted even though verification returns an error.

The stand and Docker nginx apply an additional per-IP limit to challenge/verify endpoints: 5 requests per
minute with a burst of 10. Excess requests return immediate `429 rate_limited`, the normal JSON error envelope
and `Retry-After: 60`. Other routes, including logout, do not consume this budget. The development mock alone
does not emulate the proxy limiter.

The cookie is opaque, `HttpOnly`, `SameSite=Lax` (`Secure` behind TLS). The client sends it with
`credentials: "include"` and never reads it. Identity, role and company always come from the session, never from
request fields.

Any `401` ends the session in the frontend: pending queries are cancelled, the whole query cache is cleared and the
user goes to `/login?redirect=…&reason=expired`. Protected navigations revalidate the session (one
`GET /api/v1/session` each), and a session of another actor seen from another tab clears the cache too.

## Origin

Requests carrying an `Origin` that is not allowed are rejected with `403 origin_not_allowed`. The mock allows
`http://127.0.0.1:5173` and `http://localhost:5173` in development and exactly the stand origin on the stand
(`FSP_MOCK_ALLOWED_ORIGINS`). The mock additionally accepts only loopback `Host` headers (`403 host_not_allowed`),
which is why nginx sends `Host: 127.0.0.1:4010`; that rule belongs to the mock.

## Requests and validation

- Bodies are `application/json` (`415 json_required` otherwise), except the PDF import (`multipart/form-data`).
- Schemas are strict (`additionalProperties: false`). A body, path or query value that breaks the schema is
  `400 validation_error` with `details: [{ path, message }]`; a repeated query parameter is
  `400 duplicate_query_parameter`; a body on an operation that takes none is `400 body_not_allowed`.
- The mock also validates its own successful responses and answers `500 response_contract_error` if one breaks the
  contract.
- The frontend sends only generated, typed requests (`openapi-fetch` with the generated `paths`); it never adds actor
  or company IDs as an authorization shortcut.

Company websites accept only absolute HTTP/HTTPS URLs (up to 2,048 characters). Company display names,
business areas and descriptions are bounded to 120, 200 and 4,000 characters. For PATCH, omission keeps
a value; explicit `null` clears `website` and `experienceMonths`. Other patch properties reject `null`.

## Errors

Every error body is `{ code, message, requestId, details }` and every response carries `X-Request-Id`. Behavior
branches on `code`; `message` is user-facing Russian text. Implemented codes are listed in
[`error-codes.json`](error-codes.json).

| Status | Meaning | Frontend behavior |
| --- | --- | --- |
| `400` | Malformed request or schema mismatch; also a wrong or expired code | Field errors from `details`, or the message |
| `401` | No or expired session | Clear the cache, go to sign-in |
| `403` | Wrong role, missing verification or consent, foreign origin | «Действие недоступно» with the message |
| `404` | Missing or someone else's record | «Не найдено»: the record is unavailable to this account |
| `409` | Conflict: cooldown, active attempt, terminal invitation or application, existing invitation or application, application not allowed yet, task not offered or already answered or reviewed, idempotency conflict, stale import | Refetch and show the current state |
| `413`, `415` | File too large or wrong media type | Explain the limit |
| `422` | Business validation (salary range, evidence policy, incomplete company, unsupported scope, downgrade) | Show the message next to the form |
| `429` | Verification budget or proxy request limit | Follow the error message and Retry-After hint |
| `5xx` | Service failure | «Сервис временно недоступен» with retry; the request ID stays visible |

Queries retry once on transport errors and `5xx` (job polling twice), never on `4xx`; mutations never retry
automatically. A failed
request never falls back to fixture data.

## Idempotency

`Idempotency-Key` is required for starting an assessment, submitting it, creating an invitation, applying to a
vacancy and answering a task. The server
scopes a key to actor, operation and path: the same key with the same body replays the stored response with
`X-Idempotent-Replay: true`; the same key with a different body is `409 idempotency_conflict`. The frontend makes one
key per intended command, reuses it when the same command is retried after a transport failure, and drops it once
the server has answered.

In Rust, the claim, command effects and serialized response commit in the same PostgreSQL transaction.
Concurrent requests wait for the same scope and then replay or conflict. A failed command rolls back its
claim and effects. Completed responses are retained for one day; an unresolved claim left by an older
release fails closed and requires investigation rather than automatic execution based on its age.

## Jobs

PDF import, source sync, assessment checks and submission answer `202` with a `Job`
(`id`, `kind`, `status`, `result`, `error`, `pollAfterMs`, `simulation`). The client polls `GET /api/v1/jobs/{jobId}`
no faster than every 250 ms, follows `pollAfterMs`, stops at `succeeded` or `failed`, and then refetches what the job
changes (sources, skills, graph, profile, the assessment). The mock publishes `activeJobId` for timer jobs.
Rust completes work inline and may return an already-terminal job with no active marker; clients must also
follow the job returned by the command. Rust jobs persist in PostgreSQL. A job belongs to the initiating
candidate; a failure carries `{ code, message, retryable }`. A source changed during synchronization yields
`stale_input`, and revoked processing consent yields `processing_consent_revoked` without applying the result.

## Pagination

`GET /api/v1/candidates` and `GET /api/v1/vacancies` take `page` (from 1) and `pageSize` (1–50, default 10) and
return `items`, `page`, `pageSize`, `total` and `hasNextPage`. Other lists are returned whole.

## PDF

- Export: `GET /api/v1/me/profile/export` returns `application/pdf`; the client reads it as a `Blob` and saves
  `fsp-profile.pdf`.
- Import: `POST /api/v1/me/profile/imports` with one form field `file` (`application/pdf`, 1 byte – 2 MiB). The
  browser sets the multipart boundary; the client validates type and size first. Errors: `400 file_required`,
  `415 pdf_required`, `413 file_too_large`, `422 invalid_pdf`. The job's preview is applied with
  `POST /api/v1/me/profile/imports/{jobId}/confirm`. Rust reads digital PDF text (20 pages / 64 KiB maximum)
  in a sandbox and validates Bifrost-proposed fields against page excerpts; optional `result.evidence` preserves
  these excerpts for review. Unknown fields retain current values. `profilePdfImport` gates this capability;
  export has no model dependency. A later profile revision returns `409 import_profile_changed` on confirmation.
  Parser/content failures are `422`, unavailable/busy parser is `503`, and model failures are `502`; none
  changes the profile. The development mock remains a disclosed fixture parser with the same revision guard.

## Knowledge tests and FSP snapshots

New assessment tasks use `execution: questionnaire` and `questions`; drafts use `answer.responses` as
`{ questionId, optionId }` entries. Legacy code/explanation fields remain for stored attempts, but must be empty
for new tests. Questions/responses remain optional on the wire for old immutable records. Correct options and
explanations appear only in final results. See [scoring/version rules](../docs/knowledge-tests.md).

Sources include optional `fspResults` for historical-response compatibility. Current Rust responses always emit
an array; old job replays may omit it. Published/pending/disqualified status, team scope and protocol references
are synthetic data from the [private FSP simulator](../docs/fsp-simulator.md). An unavailable source produces an
error without erasing the last successful snapshot or changing its timestamp. It never changes a grade.

## Time

Timestamps are ISO 8601 UTC. `GET /api/v1/meta` returns `serverTime`; countdowns (code lifetime, `nextEligibleAt`)
are computed from it, not from the browser clock. The mock clock starts at `2026-10-05T12:00:00.000Z` and moves only
through `/__mock__/clock`.

## Metadata and capabilities

`GET /api/v1/meta` (no session needed) returns the contract version, `mode`, `serverTime`, `scenario`,
`capabilities`, `assessmentPolicy` (cooldowns, AI allowance, supported scopes), the competition catalog and the
skill catalog. The frontend loads it before rendering a workspace, takes skill names and competitions from it, hides
features whose capability is off and shows the «Демо» dialog while `mode` is `mock`.

## Privacy projections and caching

Employer responses about candidates, including applications, are always the anonymous projection; contacts come
only from `GET /api/v1/candidates/{candidateId}/disclosure`. Every private query key in the frontend starts with
`[role, actorId, companyId]`, so data of different actors never share cache entries, and the whole cache is cleared
on sign-in, logout, a `401` or an actor change.

## Screens and operations

| Route | Operations |
| --- | --- |
| `/login` | `getMeta`, `getSession`, `createEmailChallenge`, `verifyEmail`, then `getMyProfile` or `getCompany` to pick the landing page |
| `/candidate/onboarding` | `getMyProfile`, `updateMyProfile`, `importMyProfile`, `getJob`, `confirmProfileImport`, `startAssessment` |
| `/candidate/overview` | `getMyProfile`, `getMySkills`, `getMySources`, `listMyAssessments`, `startAssessment`, `exportMyProfile` |
| `/candidate/profile` | `getMyProfile`, `updateMyProfile`, `getMySkills`, PDF import operations |
| `/candidate/sources` | `getMySources`, `getMyProfile`, `connectMySource`, `syncMySource`, `getJob` |
| `/candidate/skills` | `getMySkills`, `getMyGraph`, `getMyProfile`, `listMyAssessments` |
| `/candidate/assessments` | `getMyProfile`, `listMyAssessments`, `startAssessment`, `listMyInterviews`, `startInterview`, `getInterview`, `answerInterview` |
| `/candidate/assessments/{id}/workspace` | `getAssessment`, `saveAssessmentAnswer`, `runAssessmentChecks`, `getJob`, `askAssessmentAssistant`, `submitAssessment` |
| `/candidate/assessments/{id}/result` | `getAssessment` (polled while evaluating), `getMyProfile`, `getMySkills`, `startAssessment`, `exportMyProfile` |
| `/candidate/tasks` | `getMyTasks`, `respondToTask`, `getMyProfile` |
| `/candidate/invitations`, `/{id}` | `listMyInvitations`, `getMyInvitation`, `viewMyInvitation`, `getMyGapPlan`, `acceptMyInvitation`, `declineMyInvitation` |
| `/candidate/vacancies`, `/{id}` | `searchVacancies`, `getVacancy`, `getMyProfile`, `createApplication` |
| `/candidate/applications`, `/{id}` | `listMyApplications`, `getMyApplication`, `withdrawMyApplication` |
| `/candidate/settings` | `getMyProfile`, `updateMyProfile`, `getMySources`, `logout` |
| `/employer/needs`, `/new`, `/{id}` | `listNeeds`, `getNeed`, `createNeed`, `updateNeed`, `deleteNeed`, `publishNeed`, `unpublishNeed`, `listApplications`, `getCompany` |
| `/employer/candidates` | `listNeeds`, `searchCandidates`, `listShortlists`, `getShortlist`, `createShortlist`, `addShortlistMember`, `removeShortlistMember` |
| `/employer/candidates/{id}` | `getCandidate`, `getCandidateGraph`, `getCandidateGapPlan`, `getCandidateDisclosure`, `listNeeds`, `getCompany`, `createInvitation`, shortlist operations |
| `/employer/shortlists`, `/{id}` | `listShortlists`, `createShortlist`, `getShortlist`, `updateShortlist`, `deleteShortlist`, `removeShortlistMember` |
| `/employer/invitations` | `listInvitations`, `withdrawInvitation` |
| `/employer/applications`, `/{id}` | `listApplications`, `getApplication`, `viewApplication`, `acceptApplication`, `declineApplication`, `getCandidate`, `getCandidateDisclosure` |
| `/employer/tasks` | `listTasks`, `createTask`, `archiveTask`, `listTaskResponses`, `reviewTaskResponse`, `getCompany` |
| `/employer/company`, `/settings` | `getCompany`, `updateCompany`, `logout` |

The candidate shell also reads `listMyInvitations` for the new-invitation badge, the employer shell
`listApplications` for the new-application badge. `getNeedGraph` and the employer's
`getInvitation` are in the contract but not used by the current screens.

## Backend compatibility checklist

The Rust stand and development mock must preserve the shared behavior for: every request
and response shape and the error envelope; cookie attributes, origin checks and role/tenant scoping (`404` for
foreign records); anonymous projections and per-company disclosure; idempotent commands and terminal conflicts;
job states and `pollAfterMs`; UTC timestamps, nullable fields and stable search order; and truthful `mode`,
`capabilities` and `simulation` flags. A mismatch is fixed by one contract change on both sides
([`README.md`](README.md)), not by casts or fallbacks.
