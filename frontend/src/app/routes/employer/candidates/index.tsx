import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { needsQuery } from '@/entities/need';
import { scopeOf, sessionQueryOptions } from '@/entities/session';
import { shortlistsQuery } from '@/entities/shortlist';
import { EmployerCandidatesPage } from '@/pages/employer-candidates';
import { GRADE_IDS, SKILL_IDS, SPECIALIZATION_IDS } from '@/shared/api';

const id = z.string().regex(/^[A-Za-z0-9_.:-]{1,100}$/);

// Malformed values fall back to safe defaults instead of reaching the API.
const searchSchema = z.object({
  needId: id.optional().catch(undefined),
  mode: z.enum(['recommended', 'bank']).optional().catch(undefined),
  specialization: z.enum(SPECIALIZATION_IDS).optional().catch(undefined),
  grade: z.enum(GRADE_IDS).optional().catch(undefined),
  categorySource: z.enum(['assessment', 'estimate']).optional().catch(undefined),
  skill: z.enum(SKILL_IDS).optional().catch(undefined),
  fsp: z.enum(['any', 'verified']).optional().catch(undefined),
  page: z.number().int().min(1).max(1000).optional().catch(undefined),
  shortlistId: id.optional().catch(undefined),
});

export const Route = createFileRoute('/employer/candidates/')({
  validateSearch: searchSchema,
  loader: async ({ context: { queryClient } }) => {
    const scope = scopeOf(await queryClient.ensureQueryData(sessionQueryOptions()));
    await Promise.all([
      queryClient.ensureQueryData(needsQuery(scope)),
      queryClient.ensureQueryData(shortlistsQuery(scope)),
    ]);
  },
  component: function CandidatesRoute() {
    const search = Route.useSearch();
    const navigate = Route.useNavigate();
    return (
      <EmployerCandidatesPage
        search={search}
        onSearchChange={(patch) => void navigate({ search: (previous) => ({ ...previous, ...patch }) })}
      />
    );
  },
});
