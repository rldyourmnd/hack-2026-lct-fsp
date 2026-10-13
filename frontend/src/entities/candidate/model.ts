import { keepPreviousData, queryOptions } from '@tanstack/react-query';
import { api, type components, type paths, requireData, type Scope } from '@/shared/api';

export type AnonymousCandidate = components['schemas']['AnonymousCandidate'];
export type CandidatePage = components['schemas']['CandidatePage'];
export type Match = components['schemas']['Match'];
export type MatchRequirement = components['schemas']['MatchRequirement'];
export type Disclosure = components['schemas']['Disclosure'];
export type CandidateFilters = NonNullable<paths['/api/v1/candidates']['get']['parameters']['query']>;

export const candidatesKey = (scope: Scope) => [...scope, 'candidates'] as const;

export const candidateSearchQuery = (scope: Scope, filters: CandidateFilters) =>
  queryOptions({
    refetchInterval: 30_000,
    refetchIntervalInBackground: false,
    queryKey: [...candidatesKey(scope), 'search', filters] as const,
    queryFn: async ({ signal }) =>
      requireData(await api.GET('/api/v1/candidates', { params: { query: filters }, signal })),
    placeholderData: keepPreviousData,
  });

/** Employer projection: anonymous by construction, regardless of any later disclosure. */
export const candidateQuery = (scope: Scope, candidateId: string, needId?: string) =>
  queryOptions({
    refetchInterval: 30_000,
    refetchIntervalInBackground: false,
    queryKey: [...candidatesKey(scope), candidateId, 'profile', needId ?? null] as const,
    queryFn: async ({ signal }) =>
      requireData(
        await api.GET('/api/v1/candidates/{candidateId}', {
          params: { path: { candidateId }, query: needId ? { needId } : {} },
          signal,
        }),
      ),
  });

export const candidateGraphQuery = (scope: Scope, candidateId: string) =>
  queryOptions({
    queryKey: [...candidatesKey(scope), candidateId, 'graph'] as const,
    // Retrying a slow inference doubles the wait; the graph offers an explicit retry.
    retry: false,
    queryFn: async ({ signal }) =>
      requireData(
        await api.GET('/api/v1/candidates/{candidateId}/graph', { params: { path: { candidateId } }, signal }),
      ),
  });

export const candidateGapPlanQuery = (scope: Scope, candidateId: string, needId: string) =>
  queryOptions({
    queryKey: [...candidatesKey(scope), candidateId, 'gap-plan', needId] as const,
    queryFn: async ({ signal }) =>
      requireData(
        await api.GET('/api/v1/candidates/{candidateId}/gap-plan', {
          params: { path: { candidateId }, query: { needId } },
          signal,
        }),
      ),
  });

/** Contact data lives only in this company-scoped resource; it is never merged into the profile cache. */
export const disclosureQuery = (scope: Scope, candidateId: string) =>
  queryOptions({
    queryKey: [...candidatesKey(scope), candidateId, 'disclosure'] as const,
    queryFn: async ({ signal }) =>
      requireData(
        await api.GET('/api/v1/candidates/{candidateId}/disclosure', { params: { path: { candidateId } }, signal }),
      ),
  });
