# Grok latency and reasoning budgets

Grok 4.7 defaults to high reasoning when the request omits `reasoning_effort`. Interactive calls now send `low`
for Grok models; operators can select `low`, `medium`, `high` or `xhigh` with
`BIFROST_INTERACTIVE_REASONING_EFFORT` (API tasks) and `INTERVIEW_REASONING_EFFORT` (interview question/evaluation).
Other models retain their provider default unless explicitly configured. Invalid overrides fail during startup.

Advisory profile reviews use `BIFROST_REASONING_EFFORT`, default `medium`; `none` means omission, not disabling
Grok reasoning. The 45-second interactive deadline, the interview's shared 45-second turn budget, 330-second review
ceiling, JSON validation and bounded responses remain unchanged. There are no automatic model retries or synthetic
fallbacks. `llm_call` events contain task, model, effort, duration and outcome, never prompts or responses. An
`incomplete` outcome includes provider/transport failure or cancellation by the enclosing deadline.

The [October 11 experiment](../backend/experiments/grok-latency-2026-10-11.json) uses only synthetic evidence and
warm prompt caches. Wrong, partial and instruction-injection interview cases retained their expected rubric state
at low effort. The [long profile fixture](../backend/experiments/grok-review-synthetic.json) took 149.26 seconds at
high and 49.93 seconds at medium. Both passed bounded output checks; the advisory grades differed (senior/middle).
This is a directional latency/format check, not a production percentile or a proof of equal assessment accuracy.
The deterministic grade and evidence rules remain authoritative; AI notes never grant a verified grade.

For a live regression, compare `llm_call` durations by task and effort in OpenObserve, separating failures from
successes. Re-run representative rubric cases before changing model, prompt, effort or token limits. An operator
can explicitly restore high effort without changing the protocol or introducing a second model path.

Primary sources: [Grok 4.7](https://docs.x.ai/developers/models/grok-4.7),
[reasoning controls](https://docs.x.ai/developers/model-capabilities/text/reasoning).
