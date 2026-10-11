# Graph-guided interview service

`fsp-interview-service` is an internal Rust HTTP service for adaptive interviews. The browser uses the authenticated
platform routes under `/api/v1/me/interviews`; the platform resolves the candidate identity and calls this service
with a private bearer token. The token never reaches the browser.
Start and answer require processing consent. Revocation prevents subsequent answers while preserving access to
the candidate's existing history. Another candidate receives 404 for an interview they do not own.

Sessions are stored in PostgreSQL as versioned JSON documents. A conditional revision update rejects stale answers,
and a partial unique index permits only one active interview per candidate. The service can restart or be replaced by
a release without losing the current question or evidence.

## Product boundary

The competency graph is the interview program, not a visualization added after the conversation. The first
`backend-middle-v1` blueprint starts with an idempotency scenario. An incomplete answer moves the frontier to an
unknown prerequisite; once prerequisites are covered, the engine can return to the unresolved composite node.
The engine, not the language model, selects the next competency.

The service stores evidence states rather than personality scores:

- `unknown`: not assessed;
- `observed`: the answer contains a relevant signal but does not satisfy the rubric;
- `partial`: several rubric elements are present;
- `confirmed`: every required rubric group is supported;
- `contradicted`: a configured LLM found an explicit incorrect claim.

Technical and behavioral nodes remain separate. The shipped behavioral branch checks three observable skills:
clarifying ambiguous requirements, resolving disagreement and giving useful feedback. After every three answers the
engine schedules an untested behavioral node, while weak technical answers still descend to missing prerequisites.
It evaluates described actions and trade-offs; it does not infer personality, emotion, honesty or protected traits.

## Runtime modes

When `BIFROST_CHAT_URL` is configured, Bifrost generates surface variants and evaluates answers against the supplied
rubric. Output is schema-checked, evidence must quote the answer exactly, and the model cannot choose the next node
or assign a platform grade. `INTERVIEW_BIFROST_MODEL` selects the model independently from the main API.
Evaluation and generation of the next question share one 45-second model budget; persistence happens only after
both succeed. A timeout leaves the revision, question and evidence unchanged, so the same answer can be retried.
The internal gateway allows 50 seconds, the platform 55 and nginx 60. A provider failure is never an offline fallback.

Without Bifrost, an explicit `offline_rubric` mode selects versioned local scenarios and checks rubric concepts on
CPU. It exists for development and deterministic demonstrations; it is not evidence of assessment validity.

Required configuration:

| Variable | Meaning |
| --- | --- |
| `INTERVIEW_API_TOKEN` | Internal bearer token; there is no insecure default in the binary |
| `DATABASE_URL` | PostgreSQL connection; alternatively `PGHOST`, `PGUSER`, `PGDATABASE`, optional `PGPORT`/`PGPASSWORD` as on the stand |
| `INTERVIEW_HOST` | Bind host, default `127.0.0.1` |
| `INTERVIEW_PORT` | Bind port, default `4040`; `4030` belongs to GitHub insights |
| `BIFROST_CHAT_URL` | Optional OpenAI-compatible Bifrost chat endpoint |
| `BIFROST_API_KEY` | Optional Bifrost bearer key |
| `INTERVIEW_BIFROST_MODEL` | Interview model; otherwise `BIFROST_MODEL`, then `openai/gpt-5-mini` |

Compose uses `demo-interview-token` only for the local demo profile.

## HTTP flow

All routes except `/health` require `Authorization: Bearer $INTERVIEW_API_TOKEN`.

```text
GET  /v1/blueprints/backend-middle-v1
GET  /v1/candidates/{candidateId}/interviews
POST /v1/interviews
GET  /v1/interviews/{id}
POST /v1/interviews/{id}/answers
```

Start payload:

```json
{
  "candidateId": "candidate024",
  "blueprintId": "backend-middle-v1",
  "maxTurns": 12
}
```

Answer payload uses optimistic concurrency:

```json
{
  "questionId": "q-1-...",
  "answer": "Использую ключ идемпотентности...",
  "responseMode": "text",
  "revision": 0
}
```

The returned question contains identical `prompt` and `delivery.speechText` fields plus an `avatarCue`. Starting or
resuming an interview opens `/candidate/interviews/{id}` instead of expanding the assessments page. The workspace
shows the question as text, voices it with configured neural speech or a Russian browser voice and can transcribe a microphone
answer with the browser speech-recognition API. The candidate can switch to text at any moment and edit the transcript
before sending it. Only the confirmed transcript reaches the interview service; raw audio is not stored.

The platform accepts speech requests only for the candidate's current question and synthesizes the server-owned
`delivery.speechText`; the endpoint cannot be used as a general text-to-speech proxy. `FSP_TTS_PROVIDER=xai` uses xAI
voice synthesis. `FSP_TTS_PROVIDER=yandex` uses Yandex SpeechKit as the domestic provider. If neither is configured,
or a request fails, the client keeps the offline-capable browser synthesis path.

## Voice and avatar decision

The implemented experience uses a code-rendered interactive avatar and optional browser voice, not generated
talking-head video. Real-time generated video adds latency, cost and an external biometric processor without improving
the evidence used by the rubric. The current delivery is:

1. The candidate answers by microphone or switches to text; microphone permission is requested only after a click.
2. Bifrost creates scenario variants and evaluates evidence when configured.
3. The browser voices `delivery.speechText` in Russian and turns speech into an editable transcript when recognition
   is supported. Browser speech services may process audio remotely; the interface explains this before recording.
4. A code-rendered avatar indicates speaking, listening and analysis states. The application receives only the
   confirmed text and declared response mode; it does not capture a camera stream or store audio.

`Permissions-Policy` allows microphone input from the same origin and denies camera and geolocation. Recording
starts only on a click. Stop finishes recognition before submission is enabled; switching to text, submitting,
changing questions and leaving the page detach old callbacks. Failed recognition or evaluation preserves the draft.
Only moving to another question clears it. Browser/vendor availability is not established by a configured capability.

`responseMode` is `voice` or `text`; the browser declares voice only after it receives a final transcript. The service
records signal strength 2 for voice and 1 for text. This is a format indicator, not authenticated proof of speech,
communication quality or candidate identity: transcripts remain editable and API clients can declare either mode.
It does not change the platform's category or hiring rank.

Candidate camera recording should remain optional, stored only with explicit consent, and never be used to infer
emotion, honesty, accent, gaze or personality. It can be added later as a replay artifact for a human employer; it
is not part of automated scoring.

Camera recording and server-side audio/media storage are absent. Browser speech recognition is optional and is not
an automated audio assessment.

## Verification

`backend/experiments/graph-interview-traversal-v1.json` records the expected effect, failure condition and observed
test set for the single graph-frontier algorithmic change. Run:

```sh
cd backend
cargo test -p fsp-interview-service
```

The platform PostgreSQL HTTP test covers ownership and consent revocation. `frontend/e2e/interview.spec.ts`
exercises speech startup failure, late callbacks, preserved drafts and retry in Chromium with a controlled speech
adapter; it does not claim to test a physical microphone or a vendor recognizer. Public acceptance uses the real
Rust/Bifrost interview and verifies the browser's microphone policy. See [stability review](stability-review-2026-10-11.md).
