import { createFileRoute } from '@tanstack/react-router';
import { hiringFlowAnalyticsQuery } from '@/entities/hiring-analytics';
import { scopeOf, sessionQueryOptions } from '@/entities/session';
import { EmployerAnalyticsPage } from '@/pages/employer-analytics';

export const Route = createFileRoute('/employer/analytics')({
  loader: async ({ context: { queryClient } }) => {
    const session = await queryClient.ensureQueryData(sessionQueryOptions());
    await queryClient.ensureQueryData(hiringFlowAnalyticsQuery(scopeOf(session)));
  },
  component: EmployerAnalyticsPage,
});
