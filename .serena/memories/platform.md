# Platform behavior

Rust/PostgreSQL implements the stand; the HTTP mock is a development behavior reference. OpenAPI contains
79 application operations, health and four mock-only controls. Update every contract consumer together.
Commands and idempotency replay commit atomically. PostgreSQL serializes email budgets, profile/source/assessment,
company/shortlist and disclosure changes. Viewed flags never replace terminal decisions. Nullable patches and
bounded fields follow the contract. Candidate/company ownership and anonymous projections are server-enforced.

Real email uses the Resend adapter in backend/crates/integration-email: random six-digit codes, 15-minute expiry,
challenge-bound HMAC digests, separate session namespace, single use and five cumulative wrong attempts.
Real-mail issuance allows one/minute and five/15 minutes per normalized address, including provider failures.
Migration 0005 adds created_at/index; never edit earlier SQL. Mandatory production config is validated before
migrations. Enabling/rotating FSP_AUTH_KEY invalidates old credentials without deleting accounts. Demo/mock delivery
remains fixed and has no mailbox-ownership assurance. In Resend mode only alexey@example.com,
hr@sever-tech.example and hr@second-company.example retain that disclosed exception. The realEmail capability describes configuration, not health.

New attempts use knowledge-v2 for every catalog specialization and grade (intern–lead) on its two catalog test
topics; topic banks knowledge-v3-*.json are shared by specializations with the same topic. The attempt id selects
5+5 of six questions per topic, so variants share at least 8/10. Passing requires 8/10 and 3/5 per topic. v1 stays immutable.
Responses/snapshots are immutable at completion; legacy code attempts are archived and never execute.
Assessment start races serialize on the candidate row. Grades, consent and response submission are transactional.
PDF import uses bounded bwrap/prlimit/Poppler extraction, page-cited Bifrost fields and revision-safe confirmation.
The contextual assistant uses saved responses and never grades. Real extraction is a startup/pre-release probe.
Foreground Bifrost calls have a 45-second deadline inside the 55-second API and 60-second nginx deadlines.
Background automatic-profile reviews run asynchronously with a 330-second client deadline; insights HTTP calls have 10 seconds.
Queued reviews check processing consent before model dispatch and again before saving a still-current estimate;
already sent requests cannot be recalled. #153 has the real PostgreSQL queued-review/revocation regression.
FSP uses a private fictional HTTP provider. Published participation counts; team results never certify skills.
Failed provider sync preserves snapshots. GitHub connects by login/link through the loopback insights service
(github-insights-v3 with the scope dimension, docs/github-insights.md): tokenless background analysis of public work (search, REST, blobless
clones) polled through a running source_sync job; developer portrait, own organizations as own work, collaboration
only outside them; ownership by bio code (OAuth optional, deferred), one proven login per candidate, evidence only
from owned/led repos, anonymous rounded employer signal, no grade/order effect.

Competence graphs are deterministic catalog/evidence projections (service/graphs.rs), not LLM output.
Graph interviews use private service 4040 and PostgreSQL optimistic revisions; start/answer require processing
consent, revocation preserves read access. Model evaluation and next question share 45 seconds before persistence,
inside the 50-second gateway, 55-second API and 60-second nginx deadlines. Failure preserves the current turn.
Interview voice/text is a client-declared format, not identity/competence evidence; it never changes grade/rank.

Limits: public study bank lacks demonstrated leak resistance/professional calibration; model suggestions never
prove qualifications. Jobs finish inline except polled GitHub analyses and process-local coalesced estimate reviews; search ranks collections in memory. No OCR, durable worker, measured
large-bank capacity or production load claim. Current behavior/limits: docs/current-state.md; latest consistency review: #138; prior product completion #31.

Verify release identity against upstream/main, /opt/fsp/current and deployed.json; #138 records final proof.

Last commit: c93cf00 (queued review consent); local backend checks passed 129 tests. Live proof is in #153.
