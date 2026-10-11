export type { FspClient } from './client';
export { api, createFspClient } from './client';
export type { ApiProblem, ErrorDescription } from './errors';
export { ApiError, describeError, fieldErrors, isApiError, requireData, requireOk } from './errors';
export * from './generated/catalog';
export type { components, paths } from './generated/schema';

export const newIdempotencyKey = () => crypto.randomUUID();
export type { Scope } from './scope';
