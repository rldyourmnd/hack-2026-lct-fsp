# API contract

The single wire contract between the frontend and the backend. Both sides implement exactly this; neither keeps a
private dialect.

| File | Contents |
| --- | --- |
| [`openapi.json`](openapi.json) | OpenAPI 3.1, contract `v0.1.0`: 79 application operations under `/api/v1`, `/health`, and four `/__mock__` development controls marked `x-mock-only: true` |
| [`error-codes.json`](error-codes.json) | Error codes the mock implements, by scope (`http`, `job`) |
| [`API_OPERATIONS.md`](API_OPERATIONS.md) | Generated operation index (do not edit by hand) |
| [`API_INTEGRATION.md`](API_INTEGRATION.md) | API behavior on the wire and in the frontend: session, origin, validation, errors, idempotency, jobs, PDF, time, metadata, screen-to-operation map, backend cutover checklist |
| [`codegen/`](codegen/README.md) | TypeScript 7 generator for the frontend's contract types and the operation index |

## Where the contract comes from

Today `openapi.json` is built from [`../mock-server/src/contract.ts`](../mock-server/src/contract.ts), the same
module the mock uses to validate requests and responses. Three checks keep the copies aligned:

- `bun run --cwd mock-server test` fails when `openapi.json` differs from the builder;
- `bun run --cwd contracts/codegen check` (also part of `bun run --cwd frontend verify`) fails when the frontend
  types or the operation index are stale;
- the frontend's and the mock's TypeScript builds fail when their code no longer matches the generated types.

## Changing the contract

A contract change is one reviewed change on a named branch that updates every side together:

1. Edit `mock-server/src/contract.ts`, and the mock's behavior and tests when semantics change.
2. Run `just contract` (or `bun run --cwd mock-server contract` and `bun run --cwd contracts/codegen generate`).
3. Adapt the frontend and the backend, then run `just verify` and the backend's own tests.

Do not keep two silently divergent dialects, add undocumented fields, or cast around a mismatch. If the backend
should become the source of the contract (for example, OpenAPI generated from Rust types), make that one explicit
migration: the backend emits `openapi.json`, the mock validates against it, and the drift checks move with it.
