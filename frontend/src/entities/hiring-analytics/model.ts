import { queryOptions } from '@tanstack/react-query';
import { api, type components, requireData, type Scope } from '@/shared/api';

export type HiringFlowAnalytics = components['schemas']['HiringFlowAnalytics'];

export const hiringFlowAnalyticsKey = (scope: Scope) => [...scope, 'hiring-flow-analytics'] as const;

export const hiringFlowAnalyticsQuery = (scope: Scope) =>
  queryOptions({
    queryKey: hiringFlowAnalyticsKey(scope),
    queryFn: async ({ signal }) => requireData(await api.GET('/api/v1/analytics/hiring-flow', { signal })),
  });
