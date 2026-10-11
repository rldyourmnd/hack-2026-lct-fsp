import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { profileQuery } from '@/entities/profile';
import { scopeOf, sessionQueryOptions } from '@/entities/session';
import { mySourcesQuery } from '@/entities/source';
import { CandidateSourcesPage } from '@/pages/candidate-sources';
import { githubReportTabs } from '@/widgets/github-insights';

const searchSchema = z.object({ tab: z.enum(githubReportTabs).optional().catch(undefined) });

export const Route = createFileRoute('/candidate/sources')({
  validateSearch: searchSchema,
  loader: async ({ context: { queryClient } }) => {
    const scope = scopeOf(await queryClient.ensureQueryData(sessionQueryOptions()));
    await Promise.all([
      queryClient.ensureQueryData(mySourcesQuery(scope)),
      queryClient.ensureQueryData(profileQuery(scope)),
    ]);
  },
  component: function SourcesRoute() {
    const search = Route.useSearch();
    const navigate = Route.useNavigate();
    return (
      <CandidateSourcesPage
        tab={search.tab ?? 'overview'}
        onTabChange={(tab) => void navigate({ search: { tab }, replace: true })}
      />
    );
  },
});
