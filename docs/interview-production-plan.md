# Graph interview production checklist

The graph interview is complete only when it is usable through the authenticated product, survives process restarts,
and is released by the same verified `main` pipeline as the rest of the platform.

## Experiment contract

- Algorithmic change: use the first unresolved prerequisite as the next interview frontier and return to the
  composite competency after its prerequisites are confirmed.
- Expected effect: an incomplete composite answer produces a question about a missing prerequisite; confirming that
  prerequisite eventually returns the interview to the composite competency.
- Failure condition: the next question ignores an unresolved prerequisite, repeats a confirmed node, or cannot return
  to the composite node after its prerequisites are confirmed.
- Machine-readable record: `backend/experiments/graph-interview-traversal-v1.json`.

Behavioral cadence is a separate experiment:

- Algorithmic change: schedule one unresolved behavioral node after each three answers without bypassing its graph
  prerequisites.
- Expected effect: a 12-turn interview reaches requirements, disagreement and feedback while technical traversal
  still descends to missing prerequisites.
- Failure condition: a behavioral node is omitted, a child is selected before its prerequisite, or technical descent
  changes.
- Machine-readable record: `backend/experiments/graph-interview-behavioral-cadence-v1.json`.

## Definition of done

- [x] Persist interview sessions with optimistic revision checks.
- [x] Keep a deterministic offline CPU model and use Bifrost when configured.
- [x] Authenticate browser users through the platform API; never expose the internal service token.
- [x] Open start and resume actions in a dedicated interview route.
- [x] Render graph progress and an accessible list alternative.
- [x] Render prerequisite edges between graph nodes.
- [x] Voice questions, transcribe microphone answers when the browser supports it and allow an immediate text fallback.
- [x] Show avatar speaking, listening and analysis states without camera capture or generated video.
- [x] Assess requirements, disagreement and feedback from answer evidence without personality or emotion inference.
- [x] Build, install, start, health-check and roll back the service in `infra/release.sh`.
- [x] Cover the service in backend, Compose and release-safety CI gates.
- [x] Pass service, mock, frontend unit and targeted browser tests plus an offline HTTP smoke test.
- [x] Validate experiment records with `python3 tools/experiments.py check`.
- [x] Deploy verified `main` and confirm the public authenticated journey (baseline `c3ddffc`, successful `fsp/deployment`; subsequent hardening is tracked in [#138](stability-review-2026-10-11.md)).
