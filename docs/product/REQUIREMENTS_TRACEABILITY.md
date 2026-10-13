# Requirements traceability

How each requirement of the organizer's specification ([`sources/organizer-spec.pdf`](sources/organizer-spec.pdf),
printed page numbers) is covered by the current code. Rules are detailed in [`BUSINESS_RULES.md`](BUSINESS_RULES.md);
screens in [`frontend/docs/SCREENS.md`](../../frontend/docs/SCREENS.md); tests in
[`frontend/docs/TESTING.md`](../../frontend/docs/TESTING.md). "Mock test" refers to
[`mock-server/test/mock-api.test.ts`](../../mock-server/test/mock-api.test.ts), "e2e" to `frontend/e2e/`.

Status: **done** — implemented in the frontend, Rust API and development mock; **done, simulated** — the workflow is implemented,
the underlying data or evaluation is a fixture; **partial** — a bounded implementation with a stated requirement gap; **not implemented**. Evidence cells list the original UI/mock
cases; PostgreSQL regressions are described in the [backend guide](../../backend/README.md#run-and-check),
and release evidence follows the [CI runbook](../../infra/ci/README.md).

The specification supplies no FSP API, schema or dataset (§2, p2; §4, p6), so every FSP record here is synthetic.

## Required MVP behavior

| ID | Requirement | Status | Implementation | Evidence |
| --- | --- | --- | --- | --- |
| OR-01 | Two role workspaces, candidate and employer (§2.1, p3) | done | `/candidate/*` and `/employer/*` behind role guards; the API checks the role of every call | e2e auth «UI-25 roles are separated…» |
| OR-02 | Registration by e-mail with confirmation (§2.2.4, p4) | done | E-mail code flow on `/login`; the Rust API sends real one-time codes through Resend, while the development mock shows them in a demo mailbox | e2e auth «UI-01 …» (two cases), unit `login-page.test.tsx`, mock test «email verification…» |
| OR-03 | Profile by form or PDF extraction: name, contacts, stack, grade, roles, experience, soft skills (§2.1, p3) | done, digital PDF | Profile form; Rust uses isolated text extraction and cited Bifrost fields with editable confirmation; the development mock keeps its fixture preview. OCR is unsupported | e2e candidate «UI-04 PDF import…» |
| OR-04 | Standardized PDF profile (§2.1, p3) | done | «Скачать PDF» on the overview and the assessment result; the server renders the current profile | e2e candidate «UI-10 UI-12 UI-05 … the PDF is real», mock test «PDF export…» |
| OR-05 | Mandatory questionnaire with industry and specialization (§2, p1; §2.1, p3) | done, answered automatically | `/candidate/onboarding`; a new candidate lands there after sign-in. Connecting GitHub or FSP answers the questionnaire from evidence (`estimate-v6`); the candidate may fill or edit it by hand, edited answers are kept, and «Заполнить анкету по источникам» rebuilds them | e2e candidate «UI-02 UI-07 …», «UI-06 a new candidate connects GitHub…», mock test «connecting GitHub builds the profile…», PostgreSQL test `connecting_github_builds_the_profile_and_an_estimated_category` |
| OR-06 | The candidate selects the grade to test (§2, p1) | done | `selectedGrade` is the target, shown apart from the verified grade | unit `qualification/ui.test.tsx`, mock test «profile category is server-owned…» |
| OR-07 | Testing with comparable difficulty and resistance to leaked tasks (§2, p1–2) | partial, revised scope | Versioned knowledge tests for all 25 specializations and 5 grades select deterministic balanced 5+5 variants from six-question-per-topic pools (topic banks shared across specializations), so any two variants share at least 8 of 10 questions; distractors are length-balanced so the longest option is correct in at most half of the questions. Public source and uncalibrated items still mean leak resistance, test-retest reliability and grade separation are not established | domain experiment `assessment-variants-v2`; e2e candidate «UI-10…», «UI-13…», «UI-14…»; mock test «assessment workflow…» |
| OR-08 | Employer-visible category from questionnaire and passed assessment (§2, p1; p4) | done, extended by owner decision | Visibility needs verified e-mail, questionnaire, a category and both consents. The category comes from a passed assessment or, by the owner's decision of 2026-10-10, from an evidence estimate of at least medium confidence; it states its source, and test-confirmed candidates rank first within a category ([`BUSINESS_RULES.md` §2](BUSINESS_RULES.md#visibility-to-employers)) | mock tests «profile category is server-owned…», «connecting GitHub builds the profile…», e2e «UI-02 UI-07…», «UI-06 a new candidate connects GitHub…», Rust contract `candidate_bank_anonymous_and_sorted` |
| OR-09 | No forced downgrade after a failure; lower or higher attempts by choice (§2, p2) | done | A failed attempt keeps the verified grade. Until a grade is verified any target can be chosen; after that, attempts below the verified grade are refused so the verified grade never drops | e2e «UI-13 a failed upgrade keeps the verified Junior grade», mock test «same-grade retry…» |
| OR-10 | Time restriction on grade changes; interval chosen by the team (§2, p2; §2.1, p3) | done | 90 days after a successful grade change, configurable; same-grade retries and failures add none | e2e «UI-15 cooldown comes from the server», mock test «same-grade retry…» |
| OR-11 | Current industry, specialization, grade and history (§2.1, p3) | done | Overview, «Профиль и цели», «Проверки» history with immutable results | e2e candidate suite |
| OR-12 | Optional FSP ID; absence of history works (§2, p2; §2.1, p3) | done, simulated | Private HTTP simulator with fictional published/pending/disqualified team or individual results, protocol references and preserved snapshots on failure | e2e «UI-06 …» (two cases), mock test «scenario reset… no-FSP…» |
| OR-13 | Company profile with description, business area, contacts (§2.1, p3) | done | `/employer/company`; invitations need a complete profile | e2e employer suite |
| OR-14 | Employer describes the need: work, stack, specialization, grade (§2.1, p3) | done | Need editor with requirements, criticality, minimum grade and evidence policy | e2e «UI-16 …», mock test «need CRUD…» |
| OR-15 | Recommended categories and candidates with reasons (§2.1, p3; p4) | done | Recommended mode grouped by match status with per-requirement reasons; «Соответствие» tab on the profile | e2e «UI-18 required coverage…» |
| OR-16 | Search the whole bank with filters: specialization, grade, category, stack, confirmed FSP (§2.1, p3) | done | «Весь банк» mode; filters for category specialization and grade, the category source (test or estimate), stack and verified FSP live in the URL | e2e «UI-17 UI-19 filters live in the URL…», mock test «connecting GitHub builds the profile…» |
| OR-17 | Ranking by confirmed profile, assessment and FSP; search by need fit with explanation (§2, p1; p4) | done | Deterministic server order (grade fit, required, desired, category strength — test above high-confidence estimate above medium —, FSP as a tie-breaker) | mock test «candidate bank is anonymous, deterministic…», Rust `matching` unit tests |
| OR-18 | Changing filters does not lose the shortlist (p4) | done | Shortlists are a persisted company resource independent of filters | e2e «UI-17 UI-19 …», «an emptied bank keeps saved shortlist entries…», mock test «shortlists persist…» |
| OR-19 | Invite a candidate with an offer without publishing a vacancy (§2, p2; §2.1, p3) | done | Invitation composer from the candidate profile; a need does not have to be published | e2e invitations suite |
| OR-20 | Mandatory salary range in RUB in every offer and vacancy (§2.2.1, p4) | done | Whole-ruble monthly gross range validated in the form and on the server; a vacancy is a published need, so it always carries the range | unit `need-form.test.ts`, e2e «UI-16 salary validation…» |
| OR-21 | Offer shows description, salary, company and employer contact before communication (§2.2.1–2, p4) | done | Candidate invitation page shows the frozen terms and the company contact before the decision | e2e «UI-20 UI-21 UI-23 …» |
| OR-22 | Accept or decline; both sides see the status of invitations and applications (§2.1, p3; §2.2.3, p4) | done | Candidate decision and employer withdrawal for invitations, employer decision and candidate withdrawal for applications; statuses and timeline on both sides | e2e «UI-22 …», «UI-24 …», «UI-28 …», «UI-29 …», unit `invitation-decision.test.tsx` |
| OR-23 | No candidate contacts for the employer before acceptance or a self-application (§2.1, p4; §2.2.2, p4) | done | Anonymous projection everywhere; contacts only through the per-company disclosure resource after the candidate accepts or applies, limited to the chosen fields | e2e «UI-20 UI-21 UI-23 …», «UI-28 …», mock tests «direct invitations… disclose only selected fields…», «applications disclose…» |
| OR-24 | A candidate sees only their own invitations and applications (§2.1, p3) | done | Server scoping answers `404` for other records | mock tests «health, request validation, session, role…», «direct invitations…», «applications disclose…» |
| OR-25 | Privacy settings, processing and publication consent, role-based access (§2.1, p3; §3.5, p6) | done | Separate processing and publication consents; disclosure per invitation and company | e2e «UI-02 UI-07 …», mock test «profile category…» |
| OR-26 | End-to-end demo: registration → questionnaire → assessment → category → search → invitation and contact (§3.3, p5) | done | Full path on the HTTPS stand; scripted in [`DEMO_SCRIPT_RU.md`](../../frontend/docs/DEMO_SCRIPT_RU.md) | e2e public «a new candidate and two companies complete the reverse-hiring flow» |

## Optional scope

| Feature | Organizer position | Status |
| --- | --- | --- |
| Vacancy browsing, self-application, application history | Desirable, not MVP (§2.1, p3) | done: «Вакансии» with filters and the candidate's fit, an application that opens only the chosen contacts, «Отклики» with statuses and withdrawal (e2e «UI-27»–«UI-30», mock tests «vacancy publication…», «applications disclose…») |
| Vacancy publication and incoming applications | Desirable, not MVP (§2.1, p4) | done: publish and unpublish a need, edit it while published, «Отклики» with the frozen terms, the match and accept or decline ([`BUSINESS_RULES.md` §10](BUSINESS_RULES.md#10-vacancies-and-applications)) |
| Recurring short employer tasks | Desirable, not MVP (§2, p2; §2.1, p3) | done: companies author short tasks and review answers; each week a qualified candidate gets one task on an evidenced skill, solves it or proposes an approach, and the answer keeps the profile fresh for employers. Review stays human-owned until an opt-in evaluator has a reference contract and measured error rates ([`BUSINESS_RULES.md` §11](BUSINESS_RULES.md#11-short-tasks-and-profile-freshness); e2e «UI-31», «UI-32», mock tests «weekly tasks…», «companies author…») |
| Public GitHub activity as an additional, explainable signal | Team choice: data from open sources (§2, p2) | done: connect by login or link, tokenless background analysis of public work on our server, ownership proof by a bio code, a developer portrait (areas, languages, stack, main projects), a 0–100 index over seven dimensions, rhythm, own projects and organizations, collaboration validated by others and integrity flags; employers see an anonymous rounded projection that never changes the grade or order ([GitHub insights](../github-insights.md); Rust unit, golden and fake-GitHub service tests, PostgreSQL job-flow test, golden contract fixtures, mock test «GitHub connects by login…», e2e «UI-06 UI-07 GitHub connects by login…») |
| Anti-fraud checks for employers, ATS integration | After MVP (§2, p2–3) | not implemented; no «verified company» badge |
| Keycloak or FSP ID sign-in | Future recommendation (§2, p2; §3.5, p6) | not implemented; e-mail sign-in only |
| Docker images and `docker-compose.yml` | Desirable, a plus at evaluation (§3.5, p5) | done: `docker compose up --build` runs the frontend behind nginx and the API; images for the frontend and Rust services (API, interview and FSP simulator), checked by `infra/ci/gate.sh compose` ([`infra/README.md`](../../infra/README.md#docker)) |
| Mobile and tablet layout | Optional (§3.5, p6) | done: responsive layout with a navigation drawer below 1024 px, checked without horizontal scroll (e2e accessibility «narrow screens reflow…») |
| In-app chat | Not required | not implemented; parties contact each other directly after acceptance |

## Outside the current code

Real email delivery is implemented through Resend; public GitHub analysis runs in a separate service with ownership
proven by a bio code, while GitHub OAuth setup remains deferred by the user. An official FSP integration and code execution
are outside the chosen scope. Measured assessment/matching quality and production security review remain limitations.
Bifrost powers the estimate review with growth advice, PDF-field analysis and contextual assistance; the competence
graph is deterministic. These are explicit product boundaries
([`BUSINESS_RULES.md` §15](BUSINESS_RULES.md#15-capabilities-and-simulation)).
