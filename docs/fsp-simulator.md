# Fictional FSP provider

The federation is **not integrated**. At the user's request, a small independent Rust HTTP service models
plausible participant results. It is not a reverse-engineered API, identity-verification service, or copy of
real participants. Every response and result carries `synthetic: true` and version `fsp-demo-v1`.

The public federation [2026 student event announcement](https://fsp-russia.ru/press/news/studencheskie-sorevnovaniya-po-sportivnomu-programmirovaniyu/)
distinguishes individual algorithmic/UAV events and product programming teams of 3–6. This informed our
scope/team fields. Event names, identities, scores and protocol references below are invented; field names
and publication states are our application design, not an official federation specification.

## HTTP boundary

- `GET /health` returns service/version/synthetic metadata.
- `GET /v1/participants/{id}/results` returns `participantId`, `registered`, `results` and version/synthetic flags.
- Results include event ID/name/discipline/start/end, individual or team scope, optional team name/size,
  publication state, optional place/score, field size, protocol reference and publication timestamp.
- Published results alone count toward `verifiedParticipation`/`competitionIds`. Pending results and
  disqualifications remain visible without qualifying participation. Registration alone proves nothing.
- IDs are ASCII letters/digits/underscore/hyphen (1–100), matched case-insensitively by the adapter.
  An unknown well-formed ID returns an unregistered participant with an empty history, not a fabricated result.

| ID | Fixture |
| --- | --- |
| FSP-DEMO-024 | Published product team result: place 3/18, 82 points, four participants |
| FSP-DEMO-031 | A fictional history for a platform and systems engineer: product hackathons 1/64 (2024) and 2/120 (2025) with a team, an individual algorithmic result 9/412 (2025), the published team result 3/18 and an individual algorithmic event awaiting publication |
| FSP-DEMO-PENDING | Team event awaiting a protocol; no participation confirmation |
| FSP-DEMO-EMPTY | Registered with no results |
| FSP-DEMO-DQ | Disqualified team record; no participation confirmation |

The application `Source.fspResults` projection is optional for compatibility with historical job snapshots;
new Rust source responses always provide an array. Details are private candidate data and are not added to
employer anonymous projections or automatically disclosed contacts. No FSP result assigns a grade or individual
skill. Existing employer participation tie-break/filter semantics remain separate from assessment qualification.

## Runtime and failure behavior

Run `just fsp-simulator` alongside `just backend`. The adapter uses `FSP_SIMULATOR_URL` (default
`http://127.0.0.1:4020`). Stand systemd binds the simulator to loopback; Docker exposes it only on its private
service network. There is no public nginx route, no credentials and no mutation/admin endpoint.
The binary has no database, user files, external calls or background workers.

The adapter allows no redirects, limits connect time to 2s, the whole HTTP call to 4s, response size to 64 KiB
and history to 50 entries. It rejects mismatched participant IDs/versions, non-synthetic responses, duplicate
competitions, inconsistent scope/team fields, invalid dates/status/place/protocol combinations and malformed JSON.

Connect and refresh both perform real HTTP I/O. Validated snapshots are stored with derived participation
and candidate evidence in one transaction. Failure preserves the previous source, grade and last successful
sync timestamp. A refresh returns a failed job; the UI shows the failure alongside the last successful snapshot.
It never changes an unavailable provider into an empty history.

Migration `0004_fsp_results.sql` adds a default empty array without rewriting existing source records. Legacy
seeded participation remains identified as older demo data until explicitly refreshed. Releases build both
binaries, restart the simulator, check its health and restore the previous unit/release on activation failure.
The development contract mock uses a typed behavior fixture; it does not replace the application's HTTP adapter.
