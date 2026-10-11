# Frontend behavior

React/strict TypeScript 7, FSD and generated contracts; Russian UI and server-owned grades/privacy/state.
Assessment draft saves queue; submission waits for acknowledgment and locks editing. Knowledge workspace uses
labelled radios and saved counts; failed tests preserve grades and final results explain knowledge-only scope.
PDF previews show cited excerpts; profile changes invalidate stale confirmation. FSP cards identify fictional
records and distinguish pending/empty/DQ/outage. GitHub connects by login/link (typos rejected at once), shows the
running analysis stage, a three-step bio-code ownership check and the report in URL-backed tabs (?tab=: overview,
activity, work, stack, paginated repositories, score, employer view); employers see GitHubSignalCard and the list
shows the GitHub index. The sources page is checked at phone width without horizontal scroll.
Deterministic graphs load only in graph view; failures show explicit retry without stale success/fallback graphs.

Login uses challenge delivery mode for real-inbox versus fixed-code instructions. The public stand deliberately
exposes three fictional seed accounts; all other addresses use Resend. Real codes never appear in a public mailbox.
Graph interviews have a dedicated route, editable text and optional browser speech. Recognition may use a vendor
service; the UI explains this before recording. Same-origin microphone policy still requires user permission.
Detach callbacks on mode/question change, submission and unmount; preserve drafts on failure; disable controls
while submitting. Controlled-browser tests prove startup errors, late callbacks and retry, not physical hardware.
Required form labels include the required marker; browser tests locate their accessible role/name accordingly.
The onboarding browser helper waits for all questionnaire radios before selecting answers.

Mock E2E covers disclosed fixtures. Public E2E creates a candidate and two fresh companies and proves actual
Rust authentication, knowledge tests, PDF export, invitation acceptance and tenant privacy. Resend mode reads
actual unique test inboxes and checks message authentication; it never bypasses verification. Public browser
traces/screenshots are disabled to avoid retaining OTPs/cookies. Data persists; tests never reset the stand.
Opt-in documentation screenshots are normally skipped; interview desktop/mobile capture uses SCREENSHOTS=1.
Public actor fixtures unpublish fresh candidates, remove fresh-company needs and close contexts in teardown,
including assertion failures. Failure-injection tests prove identity guarding and unrelated-data preservation.
Private history remains; hard process termination can prevent cleanup. Use current CI reports for test counts.
Both live domain scenarios and owner real-email login passed; independent read-only UI/log checks passed.
Live/demo instructions and CI matrix were reconciled in #50; capture screenshots from scroll position zero.
Live verification receipts: #50 (consistency), #67 (GitHub insights); earlier full product/browser evidence: closed issue #31.

Latest review: #138 / docs/stability-review-2026-10-11.md; verify live identity rather than trusting old receipts.
