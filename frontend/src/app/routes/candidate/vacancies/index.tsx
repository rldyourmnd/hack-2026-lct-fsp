import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { scopeOf, sessionQueryOptions } from '@/entities/session';
import { vacancySearchQuery } from '@/entities/vacancy';
import { CandidateVacanciesPage, vacancyFilters } from '@/pages/candidate-vacancies';
import { GRADE_IDS, SKILL_IDS, SPECIALIZATION_IDS } from '@/shared/api';

// Malformed values fall back to safe defaults instead of reaching the API.
const searchSchema = z.object({
  specialization: z.enum(SPECIALIZATION_IDS).optional().catch(undefined),
  grade: z.enum(GRADE_IDS).optional().catch(undefined),
  workMode: z.enum(['remote', 'hybrid', 'office']).optional().catch(undefined),
  skill: z.enum(SKILL_IDS).optional().catch(undefined),
  page: z.number().int().min(1).max(1000).optional().catch(undefined),
});

export const Route = createFileRoute('/candidate/vacancies/')({
  validateSearch: searchSchema,
  loaderDeps: ({ search }) => search,
  loader: async ({ context: { queryClient }, deps }) => {
    const scope = scopeOf(await queryClient.ensureQueryData(sessionQueryOptions()));
    await queryClient.ensureQueryData(vacancySearchQuery(scope, vacancyFilters(deps)));
  },
  component: function VacanciesRoute() {
    const search = Route.useSearch();
    const navigate = Route.useNavigate();
    return (
      <CandidateVacanciesPage
        search={search}
        onSearchChange={(patch) => void navigate({ search: (previous) => ({ ...previous, ...patch }) })}
      />
    );
  },
});
