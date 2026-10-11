# Current system state

Reviewed on 2026-10-11 against source, private CI and the live stand. Issue
[#54](https://github.com/KirpaDmitriy/sport-programming-federation/issues/54) records the owner no-backup/no-mirror
policy; PR [#67](https://github.com/KirpaDmitriy/sport-programming-federation/pull/67) records the GitHub insights
delivery and its live check; #50 is the earlier consistency review. The [October 11 review](stability-review-2026-10-11.md) tracks the subsequent interview and acceptance fixes. This page describes behavior; verify the current
release identity as described below instead of treating a SHA in documentation as the latest release.

## Implemented and verified

| Area | Current behavior | Source / proof |
| --- | --- | --- |
| Runtime | https://nddev.link; Rust API, PostgreSQL persistence, nginx, Cloudflare Full (strict), canonical IP redirect | [Infrastructure](../infra/README.md), real public browser acceptance |
| Authentication | Resend OTP mail, random six-digit codes, challenge/session HMAC separation, durable issuance/attempt limits, secure session cookies; three explicitly disclosed seed addresses retain fixed demo codes | [Email adapter](../backend/crates/integration-email/src/lib.rs), PostgreSQL HTTP regressions, owner real-mail login |
| Privacy and writes | Role/company scoping, anonymous candidate projections, selected disclosure, atomic idempotency, guarded terminal decisions | [Business rules](product/BUSINESS_RULES.md), Rust HTTP/storage and two-company browser scenarios |
| Catalog | `catalog-v2`: five grades, 25 specializations and 255 skills (including 1C, ERP/SAP, analysis, design) in one data file shared by Rust, the contract and the mock; grouped selects and a searchable stack picker; a rollback guard refuses releases that cannot read catalog-v2 data | [Business rules](product/BUSINESS_RULES.md), catalog tests, release-safety |
| Automatic profile | Connecting GitHub and/or FSP answers the questionnaire and gives an estimated category (`estimate-v6`: directions, a 0–100 seniority score with level-framework factors including scope and complexity, grade bands, experience caps that stated experience may lift but never lower, notes, a growth plan with points and focus skills, confidence; outdated estimates refresh when the profile opens); «Заполнить анкету по источникам» rebuilds the answers and two answers (total experience, role) refine the estimate; Bifrost reviews with high reasoning, gives growth advice and a bounded second opinion; edited answers are kept; categories state their source, test-confirmed ones rank first; a «Подтверждение категории» filter for employers | [Automatic profile](auto-profile.md), [Business rules](product/BUSINESS_RULES.md#automatic-profile), domain, PostgreSQL, HTTP, mock and browser tests |
| Assessment | Versioned knowledge questions for every catalog specialization and grade (intern–lead; topic banks shared across specializations, balanced 5+5 variants per attempt), deterministic scoring, saved drafts, immutable results, preserved grade on failure | [Knowledge tests](knowledge-tests.md), component/HTTP/browser regressions |
| Graph interview | Authenticated candidate route, PostgreSQL revision-guarded sessions, prerequisite traversal, optional Bifrost evaluation, explicit consent checks, one 45-second model budget, optional browser speech with text fallback | [Interview service](interview-service.md), HTTP/timeout/browser regressions and public acceptance |
| GitHub insights | Connect by login or link; a separate loopback Rust service analyzes public work in the background without a token (search, REST, blobless clones) into a developer portrait, an explainable index over seven dimensions, rhythm, own projects and organizations, collaboration and integrity flags; ownership by bio code; employers get an anonymous rounded projection | [GitHub insights](github-insights.md), unit, golden, fake-GitHub service and PostgreSQL job-flow tests, live tokenless calibration on public accounts |
| PDF and assistance | Actual bounded isolated text extraction and cited Bifrost import; contextual assistant calls Bifrost; competence graphs are deterministic projections of stored evidence | [Backend runtime](../backend/README.md#pdf-import-runtime), exact-systemd PDF probe, earlier live feature evidence in #31 |
| FSP | Separate private HTTP simulator with explicit fictional provenance and preserved snapshots on outage | [FSP simulator](fsp-simulator.md), HTTP and browser evidence |
| Delivery | Four private-server CI gates, checked personal → dev → main PRs, exact-main deployment and actual Resend browser acceptance | [CI runbook](../infra/ci/README.md); no GitHub Actions dependency |
| Recovery | Locked immutable releases, schema/runtime compatibility guards and tested activation rollback; backups disabled by owner policy | [Operations policy](../infra/README.md#backups-and-recovery), activation fault tests |

Capability flags report configuration, not external-provider health. The development HTTP mock and its
fixed-code local mode do not prove live behavior. The public stand also deliberately exposes three synthetic demo accounts; all other addresses require real email. The public browser suite
uses actual Rust/PostgreSQL and real received email, creates fresh actors and never resets the stand.

## Explicit remaining limits

- GitHub OAuth setup is deferred by the owner; GitHub ownership is proven by a bio code instead. The GitHub index is
  a transparent rule set over public evidence, not a calibrated predictor of performance, and it never confirms a
  skill or changes matching. Official FSP integration and arbitrary candidate code execution
  are outside the chosen scope; the fictional provider and knowledge tests are intentional.
- The published study bank is not professionally calibrated or resistant to leaked answers. Results do not
  certify practical experience; LLM suggestions never decide grades.
- Interview evidence is not a calibrated hiring score. `responseMode` is a client declaration, and the displayed voice/text signal is not authenticated audio evidence. Browser speech may use a vendor service; microphone hardware and recognition availability require a compatible browser.
- Digital PDFs only: OCR is unsupported. External mail/LLM providers can fail; failures remain visible.
- FSP source jobs finish inline; a GitHub analysis is a job advanced by polling against the insights service, whose
  queue and six-hour cache live in memory (a restart re-runs analyses). All analyses share the server's anonymous
  GitHub search budget. Background estimate reviews are process-local and coalesce repeated updates per candidate; an interrupted or failed review can be requested again by recalculating. Candidate ranking still reads collections in memory. There is no durable worker,
  measured large-bank/load capacity, high availability or comprehensive production security certification.
- Backups, snapshots and redundant repository mirrors are disabled by explicit owner policy. Existing backup
  archives were deleted; do not reintroduce them. There is no external alerting/APM deployment;
  operators inspect systemd/nginx and private CI logs.
- CI admits only trusted maintainers of the private repository. Docker access is root-equivalent. Repository
  branch protection is unavailable to this account, so maintainers must follow the documented PR gates.

These are the system's current boundaries, not claims that the omitted capabilities exist. New capacity or
product requirements need their own concrete acceptance criteria before changing the architecture.

## Verify release identity and health

1. Fetch canonical `upstream` and compare each permanent local branch with its corresponding canonical remote.
   The old GitHub mirror is retired and must not be updated.
   Preserve their distinct merge history; never reset all branches to a common SHA.
2. Compare `upstream/main`, `/opt/fsp/current` on the stand and
   `/var/lib/fsp-ci/deployed.json` on server-gha-runner-1 (the CI host since 2026-10-11; server-gha-runner-2 was retired). The accepted-release marker is written only after
   deployment and the real public browser journey pass; check `fsp/deployment` as well as the four build gates.
3. Inspect active API/interview/simulator/GitHub insights/LLM/nginx/PostgreSQL units (`curl 127.0.0.1:4030/health` on the stand), recent service/nginx errors, disk space, certificate
   renewal and confirm both `fsp-backup` units are masked/inactive. No public capability flag substitutes for a real integration check.
4. Use [TESTING.md](../frontend/docs/TESTING.md) for suites and private real-mail acceptance configuration.
   Preserve provider credentials, OTPs and cookies outside git; private evidence lives in `.fsp-infra/`.

Earlier delivery ledgers: #16 (stability), #23/#27 (live browser fixes), #31 (knowledge/PDF/FSP),
#38 (domain/mail), #40 (private CI), #54 (no-backup policy), #67 (GitHub insights). Their plans and screenshots are dated evidence; current source wins.
