# Contributing to the frontend

Rules for changing the React application. Repository-wide rules are in [`../CONTRIBUTING.md`](../CONTRIBUTING.md); the domain
is in [`../docs/product/BUSINESS_RULES.md`](../docs/product/BUSINESS_RULES.md); the structure is in
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Boundaries

- The API decides grades, eligibility, matching, order and access. The UI renders the returned values and offers the
  actions the server allows; it never derives a grade or a rank.
- Use only generated contract types (`bun run codegen`); never hand-write DTOs or edit `src/shared/api/generated/`.
  A contract change goes through [`../contracts/README.md`](../contracts/README.md).
- Call `/api/v1` through the shared client. `/__mock__` is allowed only in development-only code behind
  `import.meta.env.DEV`; `scripts/check-bundle.ts` must stay green.
- Employer views use the anonymous projection and read contacts only from the disclosure resource. Never fetch
  private data and hide it in the UI.
- An API error stays an error: no silent fallback to fixtures or cached success.

## Structure

- Feature-Sliced Design: `app → pages → widgets → features → entities → shared`, slice access through `index.ts`, no
  imports between slices of one layer. `bun run lint` checks it.
- Route files only validate search parameters, guard, load and render the page. Server state lives in TanStack
  Query, URL state in validated search parameters, drafts in TanStack Form.
- Every private query key starts with the scope from `useScope()`. Clear the cache on actor changes.
- Show success only after the server confirms; keep input on recoverable errors; refetch on `409`.
- Idempotent commands keep one key per intended command and reuse it only for transport retries.

## Interface

- User-facing text is Russian, identifiers and engineering documentation English. Use the vocabulary in
  [`docs/DESIGN_SYSTEM.md`](docs/DESIGN_SYSTEM.md) and label synthetic data.
- Build from `src/shared/ui` and the tokens; no literal colors, remote fonts or images.
- Every view has loading, empty and error states. Every visible control works; capabilities reported as off have no
  controls.
- Semantic HTML, labelled controls, keyboard access, visible focus, a list alternative to graphs, no horizontal page
  scroll on narrow screens.
- Comments only for decisions that are not obvious from the code.

## Checks

For behavior changes, run `bun run verify` and `bun run e2e` (or `just verify` and `just e2e` from the root). Add or update
tests with each behavior change ([`docs/TESTING.md`](docs/TESTING.md)), refresh screenshots and
[`docs/SCREENS.md`](docs/SCREENS.md) when a screen changes, and report real results, including failures and skipped
checks. For documentation-only changes, validate references and run `git diff --check`.
Work on a feature branch and merge through `danil`, `dev` and `main`, following the repository contribution guide.
