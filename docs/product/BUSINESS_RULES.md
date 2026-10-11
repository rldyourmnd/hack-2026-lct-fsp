# Business rules

The rules implemented by the Rust [application/domain](../../backend/README.md) and the development mock
([`domain.ts`](../../mock-server/src/domain.ts)). The frontend renders server decisions and does not recompute
grades, eligibility, matching or access. Differences in simulation and execution are stated explicitly below. Wire fields and enums are defined in
[`contracts/openapi.json`](../../contracts/openapi.json); transport rules (session, errors, idempotency, jobs) are in
[`contracts/API_INTEGRATION.md`](../../contracts/API_INTEGRATION.md). Russian interface labels are quoted where they
name a state.

## 1. Accounts and access

- Two roles, `candidate` and `employer`. An account has one role, fixed at registration: asking for a code or
  verifying with the other role fails with `409 role_mismatch`.
- Registration and sign-in are the same passwordless flow: request a challenge for e-mail + role, enter the
  six-digit code, receive a session. A new e-mail creates an account: a candidate gets an empty, unverified profile
  with the alias «Кандидат №NNN» and no consents; an employer gets a new, empty company whose contact e-mail is the
  sign-in address.
- A code is valid for 15 minutes of server time, works once (`409 challenge_used`) and locks after five wrong
  attempts shared across unused live challenges for that email (`429 challenge_locked`); a wrong or expired code is `400 invalid_code` / `400 challenge_expired`.
- Real email mode sends through Resend and allows one issuance per minute, at most five in fifteen minutes
  per normalized address, including provider failures. Verification codes use random digits and keyed,
  challenge-specific hashes; session hashes have a separate namespace. Mock/demo codes and the three exact seeded public addresses remain disclosed fixtures; they do not prove mailbox ownership.
- The session is an opaque `HttpOnly`, `SameSite=Lax` cookie. Without it every protected call is `401`.
- A candidate reaches only their own profile, sources, skills, assessments, jobs and invitations; an employer only
  their own company, needs, shortlists and invitations. Someone else's record answers `404`, so its existence is not
  revealed; a call for the other role answers `403 role_forbidden`.

## 2. Candidate profile

The catalog (`backend/crates/domain/data/catalog.json`, version `catalog-v2`) is shared by the Rust API, the contract
generator and the development mock: five ordered grades (стажёр, джуниор, мидл, сеньор, лид), 25 specializations in
eleven groups with the two topics of their knowledge test, and 255 skills in 17 categories, including the 1C ecosystem,
ERP/SAP, analysis, design and management. `/meta` returns it with labels; requirements, stacks and filters accept any
catalog skill. GitHub evidence maps languages and technologies onto catalog skills.

| Owner | Fields |
| --- | --- |
| Candidate (editable) | `name`, `industry` (only `it`), `specialization` (one of 25 catalog specializations), `selectedGrade` (`intern`/`junior`/`middle`/`senior`/`lead`), `stack` (up to 12 catalog skills), `teamRole`, `experienceMonths` (`null` unknown, `0` none, up to 1200), `experience`, `softSkills` (up to 12, self-assessed), `consents` |
| Server | `verifiedSpecialization`, `verifiedGrade`, `gradeConfirmedAt`, `category`, `estimate`, `autoFields`, `onboarding`, `assessmentEligibility`, `revision` |

- `selectedGrade` is the target of the next assessment («Цель»). Changing it never changes `verifiedGrade`;
  only an assessment result does.
- The questionnaire is complete when name, specialization, selected grade, at least one stack item, team role and
  experience are filled.

### Automatic profile

A candidate does not have to type anything: connecting GitHub and/or FSP builds the profile. The rules are
`estimate-v6` in [`backend/crates/domain/src/estimate.rs`](../../backend/crates/domain/src/estimate.rs); the plan of
record is [`docs/auto-profile.md`](../auto-profile.md).

- Every finished source write (connecting FSP, a finished GitHub analysis, a sync) recomputes `estimate` from all
  connected sources in the same transaction. A running GitHub analysis keeps the previous estimate.
  `POST /me/profile/estimate` recomputes on request (`409 source_not_connected` without a finished source) and
  re-analyzes a GitHub report of an earlier analysis version; that job applies its estimate when it ends.
  `?answers=replace` («Заполнить анкету по источникам») first hands every answer except an edited name back to the
  evidence; a released field the estimate cannot answer returns to the candidate. A PATCH that changes the stated
  experience or team role recomputes the estimate at once.
- `estimate` holds the leading specializations with shares, a 0–100 seniority score with its factors (eight GitHub
  dimensions weighted scope 20, tenure 15, own projects 15, practices 15, collaboration 15, recognition 10,
  engagement 5, regularity 5; or stated experience up to 30 without GitHub; FSP results up to 15), the grade band
  (< 20 стажёр, 20–39 джуниор, 40–59 мидл, 60–79 сеньор, ≥ 80 лид), the confidence, the sources, `insufficient` when
  there is too little evidence, `notes` and the answers it suggests. Сеньор needs 24 months of experience and лид 48,
  counted in months: the longer of public work (from the first analyzed commit or the first year with ten active
  days) and stated experience. Stated experience never lowers the score: it replaces the tenure dimension only when
  longer than the public record (full at 72 months), and a grade only it unlocks keeps the confidence at medium.
  Лид also needs work with people: a collaboration dimension of at least 50 or a leading team role the candidate
  typed (тимлид, руководитель, архитектор, стафф, принципал, главный, lead, head, principal, staff, CTO, director); otherwise the band gives
  сеньор, and a lead that only the stated role unlocks keeps the confidence at medium.
  Stated experience is `experienceSince` (the year the candidate started in IT; whole years up to the current year)
  or typed/imported `experienceMonths`; it is never filled from public tenure, and earlier auto-filled values are
  cleared (`estimate-v6`). `notes` explain an applied cap, tenure taken from the questionnaire, an outdated GitHub report and what the next
  band needs. (`estimate-v2` blended stated experience into the score; `estimate-v1` counted calendar years.)
- The category uses the candidate's specialization when it is among the estimate's leading specializations,
  otherwise the estimate's leader.
- `growth` lists what raises the grade (tenure and work-with-people conditions, the GitHub factors with the most
  points left as concrete actions, FSP, the optional test) and `focusSkills` up to five catalog skills of the
  specialization the evidence lacks. FSP results together add at most 30 to specialization scores. `GET /me/profile`
  refreshes an estimate of an earlier version; `POST /me/profile/estimate` re-syncs FSP sources first.
- The estimate answers the questionnaire fields that are empty, that an earlier estimate filled (`autoFields`), and
  the specialization and target grade while the questionnaire is incomplete. A test-confirmed category keeps its
  specialization and target grade. A field the candidate changes leaves `autoFields`, and estimates never overwrite
  it again; a PATCH that resends an unchanged value keeps the field automatic. Soft skills are answered from public
  evidence only (documentation, descriptive commits, led projects, reviews, accepted outside work, issues, team
  competitions). A new account starts as «Новый кандидат», which the public GitHub name may replace.
- When Bifrost is configured, the LLM receives the anonymized evidence (no login, names, links or organizations:
  areas, technologies by layer, led projects' descriptions, all metrics, factors, notes, public and stated tenure),
  grades it with a rubric by scope, complexity, ownership, quality, influence and tenure, and gives a second opinion in the background after the source write commits (up to 60 seconds), so a slow model
  never delays connecting a source. The review applies only while the reviewed estimate is still the stored one and
  processing consent holds. It may choose between the two leading specializations when their shares differ by at
  most ten points and may rewrite the experience and team-role texts (texts with links or addresses are dropped). It
  never sets the grade: a disagreement lowers high confidence to medium (never below, so a review never removes the category) and is shown. A restart loses a pending
  review; «Пересчитать» asks again.
- The mock applies the golden estimates the Rust rules write to
  [`contracts/fixtures/profile-estimates.json`](../../contracts/fixtures/profile-estimates.json) for its fixed GitHub
  report and FSP result; it ignores experience the candidate typed but mirrors `answers=replace` and the category
  choice among supported specializations.

### Visibility to employers

A profile appears in employer search only when all five conditions hold; `onboarding.blockers` names the missing
ones and the UI lists them on the overview, profile and settings pages:

| Condition | Blocker |
| --- | --- |
| E-mail verified | `email` |
| Questionnaire complete | `questionnaire` |
| A category: a passed assessment, or an estimate with a specialization, a grade and at least medium confidence | `category` |
| Processing consent | `processing_consent` |
| Publication consent | `publication_consent` |

`category` is `{ specialization, grade, eligible, source, confidence }`: the verified values with
`source: assessment`, otherwise the estimate with `source: estimate` and its confidence; `eligible` equals visibility.
The organizer's specification derives the category from the questionnaire and a test; the owner chose to let the
estimate assign it too, so every category states its source, test-confirmed categories rank first (section 7), and
the UI never calls an estimate «подтверждено».

### Consents

- **Processing** («Согласен на обработку данных») is required for PDF import and its confirmation, connecting and
  syncing sources, and for starting, saving, checking, submitting and discussing assessments
  (`403 processing_consent_required`). The mock cancels unfinished timer jobs with `processing_consent_revoked`; Rust rechecks consent before
  applying external source-sync results. Rust assessment/import work completes inline.
- **Publication** («Показывать профиль работодателям») only controls appearance in search. The two consents are
  independent and saved separately; the UI shows a change only after the server confirms it.

## 3. Sources and evidence

- Two providers, GitHub and FSP. In the mock both are synthetic (`synthetic: true`, links under
  `example.com/fixtures`), and the UI labels them «Демо-источник».
- Connecting a source requires processing consent. The mock starts a `source_sync` timer job at once.
  Rust commits the FSP source inline; a GitHub analysis runs for minutes, so connecting or syncing GitHub records a
  running `source_sync` job in `Source.activeJobId`, and polling the job applies the finished report in one
  transaction (`Source.github.analysis` shows the stage and progress meanwhile). In both implementations there
  is no manual repository selection. A connected source can be synced again (`409 source_not_connected` otherwise);
  both return the same running job for overlapping syncs. Rust detects a source changed during its
  provider call and returns a failed `stale_input` job instead of overwriting newer data. Disconnecting is not supported.
- GitHub connects by login, `@login` or a profile link (`422 github_login_invalid` for repository links and other
  hosts; `422 github_account_not_found` at once for a mistyped login or an organization account). Updating within
  six hours of the last analysis reuses its report and re-checks the bio, so ownership is confirmed in seconds. The insights service analyzes **public** data only, without a token, on our
  server, and returns a report (`Source.github.report`): a developer portrait (summary, areas of work with evidence,
  languages by the candidate's changed files, stack by category, topics, main projects), a 0–100 index with band and
  confidence, eight dimensions with metrics (including scope and complexity), a commit calendar, yearly public commits, an estimated work rhythm in the
  author's own time zone, own projects and organizations, collaboration validated by other people, repository
  practices and integrity flags. Work in the candidate's own organizations counts as own work; collaboration counts
  only work accepted or reviewed by others outside them. Time per active day and week is an explicitly labelled
  estimate from commit timestamps, not worked hours. The method is in [GitHub insights](../github-insights.md).
- Ownership: a login alone is a claim. Placing the candidate's code (`Source.github.ownership.code`) in the GitHub bio
  and syncing, or an OAuth token of the same account, proves it; a proven login belongs to one candidate
  (`409 github_account_taken`). Employers see whether ownership is proven.
- Rust derives catalog technologies as `evidence` only from repositories the candidate owns or substantially
  develops, preserves assessment confirmations and removes superseded GitHub support. The mock uses synthetic
  evidence for empty profiles. Sources never create a confirmed grade: a technology in a repository is not proof of
  personal skill or level, and the GitHub index never changes matching or order.
- FSP is optional. Rust calls the private HTTP simulator on both connect and sync. Fictional results distinguish
  published, pending, disqualified and empty histories, individual and team scope, event dates and protocol references.
  Only published results count as participation for filtering/ranking; team placement never confirms individual skills
  or grades. Failed synchronization preserves the previous snapshot and timestamp alongside the error.
  See [FSP fixtures and contract](../fsp-simulator.md).

### Skill states

| State | Label | Meaning |
| --- | --- | --- |
| `confirmed` | «Проверено» | Confirmed by a knowledge test; `evidenceGrade` is the assessed grade |
| `evidence` | «Есть свидетельства» | Source material shows use; the level is not checked |
| `observed` | «Найдено в работе» | The technology was found in a project; the level is not confirmed |
| `insufficient` | «Недостаточно данных» | No usable evidence; a data gap, not a verdict |

States are categories, not percentages. Each skill lists its evidence items (source `github`/`fsp`/`assessment`,
title, reference, date, individual or team scope, link). The competence graph is built from stored facts only, so
every skill appears once: the category (`role`, with «подтверждено тестом» / «оценка по открытым данным») →
concepts (`concept`, catalog skill categories such as «Бэкенд и API») → technologies (`skill`) with their level in
`detail` — «Тест · Мидл», «Основная · N проектов» (10+ repositories), «Регулярно» (3–9), «Встречается» (1–2),
«Указан в анкете» or «Нужно подтверждение». Sources (`source`): GitHub supports the concepts it evidences, a test
supports the category and its concepts, FSP supports the category only. Employers see no login, links, places or
evidence titles. The frontend draws server graphs as they are.

## 4. Assessments

- Supported scope: every catalog specialization at every grade, intern to lead
  (`meta.assessmentPolicy.supportedScopes`). Each attempt has ten single-choice questions selected
  deterministically from a larger grade pool: five on each of the specialization's two catalog test topics (for
  example Rust and PostgreSQL for backend, the 1C language and 1C queries for 1C). Topic-bank questions serve every
  specialization tested on that topic. The immutable `knowledge-v2` snapshot is checked against the server bank
  before grading. Correct options and explanations are returned only in the final result.
- Passing requires at least 8/10 overall and 3/5 in each subject. This measures knowledge on these questions,
  not practical development experience. The bank is public study material, not a secure certification exam.
- AI and references are allowed, including external tools (`externalAiUsed`). No code is written or executed;
  `task.execution` is `questionnaire`. [Versioning and scoring](../knowledge-tests.md) define the full policy.

### Eligibility

`assessmentEligibility` on the profile decides the primary action; the server repeats every check when an attempt
starts.

| Reason | When | Start error |
| --- | --- | --- |
| `onboarding_incomplete` | E-mail unverified, questionnaire incomplete or no processing consent | `409 onboarding_incomplete` |
| `active_assessment` | A questionnaire is `draft`, `in_progress`, `submitted` or `evaluating`; one active test at a time | `409 assessment_active` |
| `unsupported_scope` | The bank holds no test for the specialization and grade (see `meta.assessmentPolicy.supportedScopes`) | `422 assessment_scope_unavailable` |
| `grade_downgrade` | The target is below the verified grade in the same specialization; such attempts are never run | `422 grade_downgrade_forbidden` |
| `cooldown` | A grade-changing attempt within the cooldown after the last successful grade change | `409 grade_change_cooldown` |

The cooldown is 90 days by default (`FSP_GRADE_CHANGE_COOLDOWN_DAYS`, 0–365) and counts from `gradeConfirmedAt`;
`nextEligibleAt` tells when it ends. A retry at the already verified grade is allowed during the cooldown.
`canStartSelected` answers for the saved target, `canStartUpgrade` for any grade-changing attempt.

### Attempt lifecycle

`draft` → (answer saved) `in_progress` → (submitted) `submitted` → `evaluating` → `passed` | `failed` |
`service_error`.

- Only `draft` and `in_progress` questionnaires are editable (`409 assessment_not_editable`). Drafts may be partial;
  submission requires one valid answer per question (`422 answer_incomplete`) and freezes the attempt. Unknown,
  duplicate or foreign question/option IDs are rejected. New tests require empty legacy code/explanation fields.
- Checks report completeness only; they do not reveal correctness before submission. Saving and submission serialize
  on the candidate so overlapping changes cannot overwrite a final result. The UI queues saves and waits for acknowledgement.
- The Rust assistant uses the task, saved responses and question through Bifrost, rechecks consent/editability after
  I/O and never grades. Failures remain errors; the development mock labels its prepared answers.
- Historical code attempts are read-only. Starting a new test retires any active legacy attempt with an archive
  explanation and no grade effect. Old results are retained without representing them as knowledge-test evidence.

### Outcomes

Rust and the ordinary mock scenario use the same versioned bank and deterministic thresholds. Explicit mock
`assessment_failure` and `service_failure` scenarios force outcomes for UI testing and are labelled simulated.

| Outcome | Effect |
| --- | --- |
| `passed` | The verified specialization and grade become the assessed ones (when there were none, the specialization differs, or the grade is not lower). `gradeConfirmedAt`, and with it the cooldown, moves only when the grade actually changed. The two assessed skills become `confirmed` at the assessed grade with an assessment evidence item appended; earlier evidence and other skills stay. |
| `failed` | The verified grade stays; rubric criteria are partial or not met; no cooldown is added. |
| `service_error` | No rubric, the grade stays, no cooldown; the job fails as `assessment_service_unavailable` (retryable) and a new attempt can start at once. |

A result records `outcome`, `previousGrade`, `confirmedGrade`, `gradeChanged`, rubric outcomes, checks, feedback and
`nextEligibleAt`. The history lists every attempt, newest first, and results never change afterwards.

## 5. Company

The company profile holds `displayName`, `businessArea`, `description`, `contactEmail` and an optional `website`.
It is `readyToInvite` when the first four are filled. Search and needs work without it, but sending an invitation
requires it (`422 company_incomplete`) and a verified employer e-mail (`403 email_not_verified`). There is no company
verification or «verified» badge.

## 6. Needs

- A need has a title, specialization, grade, work mode (`remote`/`hybrid`/`office`), salary, description and 1–5
  requirements. Each requirement has a skill, criticality (`required`/`desired`), a minimum grade and an evidence
  policy.
- Salary is a whole-ruble range from 1 to 100 000 000 with `currency: RUB`, `period: month`, `taxBasis: gross`
  («До вычета налогов»); currency, period and tax basis are fixed, not chosen.
- The evidence policy accepts any of knowledge tests, GitHub work and FSP competitions; for FSP either
  `system` (any catalog competition that covers the skill) or `selected` (listed competition IDs).
- Validation on create, update and again when an invitation is sent (all `422`): `from ≤ to`
  (`salary_range_invalid`), unique skills (`duplicate_skill`), at least one required skill
  (`required_skill_missing`), at least one accepted source per requirement (`empty_evidence_policy`), known
  competitions (`unknown_competition`), an empty list with `system` (`ambiguous_competition_policy`), at least one
  competition with `selected` (`competition_required`).
- A need is private until the company publishes it (section 10); invitations never need a vacancy. Deleting or
  editing a need does not change invitations already sent or applications already received.

## 7. Search and matching

- The bank contains only profiles visible to employers (section 2).
- **Recommended** mode needs a need of the company (`422 need_required`) and keeps candidates whose category
  specialization equals the need's and whose category grade is at least the need's. **Bank** mode shows everyone
  visible.
- Filters: category specialization, exact category grade, the category source (`categorySource`: `assessment` or
  `estimate`), a skill in any state except `insufficient`, and verified FSP participation. Pages of 1–50 (default 10).

### Requirement result

| Status | Label | Rule |
| --- | --- | --- |
| `meets` | «Подтверждено» | The skill is `confirmed` by an assessment, the policy accepts assessments, and `evidenceGrade` ≥ the minimum grade |
| `not_evidenced` | «Недостаточно данных» | No skill or `insufficient` |
| `needs_verification` | «Есть основания, нужна проверка» | Any other case: evidence or observation only, a lower confirmed grade, or a confirmation the policy does not accept |

Only an assessment confirmation satisfies a requirement. GitHub work or FSP participation never does on its own,
even when the policy accepts those sources; they feed the gap plan instead.

A candidate is `missing_required` («Не хватает свидетельств по обязательным требованиям») when any required item is
`not_evidenced`, `meets_required` («Подходят по обязательным требованиям») when every required item meets and the
grade fits (same category specialization, grade at least the need's), otherwise `needs_verification` («Требуют
дополнительной проверки»). Desired items never compensate for a missing required one. The result carries counts, the
per-requirement reasons, a sentence of explanation and `algorithmVersion: fixture-rules-v1`.

### Order

The server sorts; the UI groups by match status and says that the order is set by the server and is not a
probability of being hired.

- With a need: grade fit, required items met, desired items met, category strength, verified FSP participation,
  candidate ID.
- Without a need: category specialization (catalog order), category grade (higher first), category strength,
  verified FSP participation, candidate ID.

Category strength: a test result 3, a high-confidence estimate 2, a medium-confidence estimate 1.

FSP participation only breaks ties; its absence never disqualifies.

### Anonymous projection

Every employer response about a candidate (search, profile, graph) contains only: ID, alias, the category
(specialization, grade, `categorySource`, `categoryConfidence`), the verified grade with its confirmation date when
there is one, for an estimated category the score and factor points (`estimate`, without explanations or texts), work modes, `availableForOffers`, skills (state, evidence grade, source kinds,
summary, evidence count, confirmation date — no evidence items), source kinds, verified FSP flag, profile freshness
(section 11), the match and the anonymous GitHub signal (`github`: index, public summary, areas of work, dimensions
with rounded metrics, ownership state, languages, technologies, rhythm bands, highlights and warnings — no login,
names, links, calendar or exact counts). It
never contains a name, e-mail, handles, repository names, links or evidence titles. A profile that is not visible
answers `404`, except to a company the candidate applied to (section 10) until the application is withdrawn.

### Gap plan

For a need, each requirement that does not meet gets an action: a knowledge test when the policy accepts
assessments and the need has a supported assessment scope, otherwise «provide accepted evidence», plus the
accepted catalog competitions when the policy accepts FSP. Employers get it for any visible candidate and their own need. Candidates get it only for
needs from their own invitations, built from the invitation's snapshot, so other needs cannot be read by guessing
IDs.

## 8. Shortlists

- Shortlists belong to a company and have a title (1–120 characters). Membership is a separate resource: adding
  requires a visible candidate, removing always works, and search filters or paging never change it.
- A member who is no longer visible stays in the list with `available: false` («Профиль недоступен»). Deleting a
  shortlist does not touch the bank.
- The UI adds and removes optimistically, rolls back if the server rejects, and updates every place that shows the
  membership.

## 9. Invitations

- **Create** (employer): a ready company, a verified employer e-mail, a valid need of the company, a visible
  candidate, a message of 1–4000 characters and an `Idempotency-Key`. There can be only one active (`sent`, `viewed`
  or `accepted`) invitation per company, candidate and need (`409 invitation_exists`).
- The invitation freezes a snapshot of the company (name, business area, description, contact e-mail) and of the
  need's terms (title, work mode, salary, description). The candidate sees the salary, the company and its contact
  before deciding.
- **Lifecycle:** `sent` → `viewed` (the candidate opens it; an explicit call, only from `sent`) → `accepted` or
  `declined`; the employer may withdraw a `sent` or `viewed` invitation. `accepted`, `declined` and `withdrawn` are
  terminal: a different action answers `409 invitation_terminal`, repeating the same one returns the record unchanged.
- **Accept:** the candidate picks at least one of name, e-mail and source links (the UI preselects name and e-mail);
  a verified candidate e-mail is required. Declining and withdrawing grant nothing.

## 10. Vacancies and applications

Self-application is the additional path next to invitations: the company publishes a need as a vacancy, a candidate
applies, and the company decides.

- **Publish** (employer): `POST /needs/{id}/publish` needs a ready company (`422 company_incomplete`), a verified
  employer e-mail (`403 email_not_verified`) and a valid need; it sets `published: true` and `publishedAt`. Repeating
  it keeps the original time. `POST /needs/{id}/unpublish` hides the vacancy again (`publishedAt: null`). Edits to a
  published need are visible to candidates at once.
- **Browse** (candidate): `GET /vacancies` lists published needs of every company, newest first, filtered by
  specialization, grade, work mode and required skill, paged like the bank. A vacancy shows the company (name,
  business area, description, contact e-mail), title, specialization, grade, work mode, salary, description and the
  requirements without the evidence policy. An unpublished or unknown vacancy answers `404`.
- **Fit:** for a candidate with a category and processing consent the vacancy carries the same `Match` as the
  employer search (section 7), computed from the candidate's category and skills; otherwise `match: null`.
- **Eligibility:** the server reports `eligibility: { canApply, reason }`. Applying needs, in this order, a verified
  e-mail (`email`), a complete questionnaire (`questionnaire`), a category (`category`) and processing
  consent (`processing_consent`); publication consent is not needed, because an application goes to one company. A
  candidate has at most one application per vacancy that is not withdrawn (`application_exists`).
- **Apply** (candidate): `POST /me/applications` with the vacancy, a message of 1–4000 characters, at least one field
  to disclose (name, e-mail, source links) and an `Idempotency-Key`. Missing processing consent answers
  `403 processing_consent_required`, an unmet condition `409 application_not_eligible`, a repeat
  `409 application_exists`. The application freezes the company and the vacancy terms (title, specialization, grade,
  work mode, salary, description); its `match` is computed against those frozen terms each time it is read.
- **Lifecycle:** `sent` → `viewed` (the company opens it) → `accepted` or `declined` by the company; the candidate may
  withdraw a `sent` or `viewed` application. `accepted`, `declined` and `withdrawn` are terminal: another action
  answers `409 application_terminal`, repeating the same one returns the record unchanged. After a withdrawal the
  candidate may apply again.
- **Contacts:** applying opens the chosen fields to that company at once (the specification's «откликнулся сам»).
  Withdrawing takes them back unless an accepted invitation from the same company still grants them. While an
  application is not withdrawn, the company also sees the candidate's anonymous profile even if it is not in the
  public bank.
- Applications are scoped like invitations: a candidate sees only their own (`/me/applications`), a company only those
  sent to it (`/applications`, optionally filtered by `needId`); anything else answers `404`.

## 11. Short tasks and profile freshness

The specification's recurring short tasks keep a profile current and give employers a fresh signal.

- **Authoring** (employer): `POST /tasks` with a title (1–160), a prompt (1–4000), a skill, a minimum grade and an
  estimate of 5–120 minutes; it needs a ready company (`422 company_incomplete`) and a verified employer e-mail
  (`403 email_not_verified`). `POST /tasks/{id}/archive` stops offering it; answers stay. A company sees only its own
  tasks with their answer counts.
- **Weekly offer** (candidate): `GET /me/tasks` returns the current period — the ISO week of the server clock, Monday
  00:00 UTC to the next Monday — and at most one task for it: an active task of any company on a skill the candidate
  has evidence for (any state but «Недостаточно данных»), at or below the category grade, not answered in an earlier
  week, chosen by rotating the eligible tasks by the week number. An answer given this week keeps that task as the
  week's task. Without a category the reason is `category`, without processing consent `processing_consent`,
  without a matching task `no_tasks`.
- **Answer:** `POST /me/tasks/{id}/responses` with `mode` (`solution` or `approach`), an answer of 1–10000 characters
  and an `Idempotency-Key`; one answer per week (`409 task_response_exists`) and only to the offered task
  (`409 task_not_offered`); processing consent is required. The answer freezes the task as offered. Only the
  candidate and the authoring company see it; it changes neither the grade nor skill states.
- **Review** (the authoring company): `POST /task-responses/{id}/review` with `accepted` or `needs_work` and optional
  feedback, once (`409 task_response_reviewed` for a different verdict; repeating the same one returns the record).
- **Evaluation boundary:** the authoring company remains authoritative. There is no automatic LLM verdict because
  authored tasks have no required reference answer or measured evaluator accuracy. Adding one would require an
  opt-in evaluation contract and a recorded false-accept/false-reject experiment, not a silent replacement of review.
- **Freshness:** the profile's and the anonymous projection's `freshness` names the latest signal — the grade
  confirmation or latest successful same-grade knowledge test (`assessment`) or a task answer the company did not mark `needs_work` (`task`) — and is `fresh` while
  that signal is at most 90 days old («Профиль актуален» / «Давно не обновлялся»). It does not change search order.

## 12. Contact disclosure

Contacts are a separate resource per company (`GET /api/v1/candidates/{id}/disclosure`): `granted: false` with no
fields, or the union of the fields the candidate chose in every accepted invitation and every application that is not
withdrawn, from that company only (name, e-mail, repository links from connected sources), with the latest invitation
and application IDs and the time of the latest grant. Every other company keeps seeing the anonymous profile. Search,
profile, graph and application responses never include contacts. There is no chat: the parties contact each other
directly, and the company's contact was visible from the start.

## 13. PDF profile

- **Export:** a real PDF of the current profile (Krilla in Rust, PDFKit in the mock, embedded Cyrillic fonts),
  file `fsp-profile.pdf`.
- **Import:** one `application/pdf` file up to 2 MiB with a valid signature and end marker starts a `profile_import`
  job; its preview is edited by the candidate and applied once with a confirmation (`409 import_not_ready`,
  `409 import_already_confirmed`). Import cannot change verified fields and needs processing consent. The mock returns a disclosed `fixture_preview`. Rust uses `poppler_bifrost` with `simulation: false`:
  digital text from up to 20 pages, 64 KiB maximum, is parsed in an isolated process and then supplied to Bifrost.
  Each extracted field carries a matching page excerpt; unknown fields preserve current profile values. Catalog
  values (specialization, target grade, stack) are accepted for the whole catalog when the excerpt names their id,
  label or a common alias as a whole word; `experienceSince` (the year the person started in IT: IT study, a first
  programming job, «программирую с …») only when the excerpt contains that year, and it replaces stated months.
  The candidate reviews and edits the preview. A profile revision change invalidates confirmation with
  `409 import_profile_changed`. Scans, encrypted files and parse/analysis failures never update the profile.
  `profilePdfImport` gates import independently of export.

## 14. Jobs

Four kinds: `profile_import`, `source_sync`, `checks`, `assessment`. The mock progresses timer jobs from queued
through running to a terminal result in about one second; restarting it clears them. Rust performs the bounded
work inline and persists terminal jobs in PostgreSQL. Clients handle both immediate completion and polling,
respect `pollAfterMs`, and invalidate the affected profile, skills, graph and sources. Jobs are private to the
initiating candidate; failures carry `{ code, message, retryable }`. No background worker is deployed.

## 15. Capabilities and simulation

`GET /api/v1/meta` reports the configured capabilities. The mock reports `mode: mock`; Rust reports `mode: live`
while still marking demo features as simulated. Real LLM, GitHub OAuth and `githubInsights` (public GitHub analysis)
flags indicate configured adapters. The stand enables Bifrost and GitHub insights and leaves GitHub OAuth unconfigured.
The public demo exposes three seeded synthetic accounts with the disclosed fixed code. The fixed code applies only
to their exact addresses and is not proof of mailbox ownership; every other address uses Resend.

The development mock uses demo email identity; Rust uses actual Resend delivery when configured. Both use fictional FSP participation. Knowledge tests have deterministic
scoring; their public bank has no measured psychometric validity. Only the development mock simulates PDF extraction
and assistant replies. Rust uses isolated Poppler extraction and Bifrost with validated evidence and no canned fallback.
Rust persists application records and sessions; its initial candidate bank is synthetic. GitHub analysis is real
when the insights service is configured; the mock returns the labelled fictional report of a synthetic account. Competence graphs are deterministic projections of catalog concepts and stored evidence; employer projections remove
private evidence. Graph structure is not a qualification verdict. Real email delivery for non-demo addresses is implemented through Resend; GitHub OAuth setup remains
deferred (ownership is proven by the bio code instead); official FSP integration and arbitrary code execution are outside the chosen product scope.

## 16. Graph interviews

The authenticated candidate can start or resume one active `backend-middle-v1` interview. PostgreSQL stores
its question, prerequisite progress, evidence and revision; competing stale answers return `409 interview_conflict`.
Starting or answering requires processing consent. After revocation, reading existing history remains allowed
but new answers are rejected. Other candidates receive 404 and employers cannot use these private endpoints.

The engine selects the next competency; Bifrost evaluates cited answer evidence and phrases the next question
when configured. The complete model turn has one 45-second budget before persistence. Failure leaves the current
question and revision unchanged. Offline mode is explicit and never substitutes for an unavailable configured model.

The browser supports text and optional speech recognition with editable transcripts. Its vendor may process audio
remotely; only submitted text and declared response mode reach the application. Voice/text signal strength is
a format indicator, not verified speech, identity or professional competence, and does not change matching or grades.
There is no camera or audio archive. See [interview service](../interview-service.md) for configuration and tests.
