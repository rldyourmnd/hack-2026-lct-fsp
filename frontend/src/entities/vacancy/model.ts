import { keepPreviousData, queryOptions } from '@tanstack/react-query';
import { api, type components, type paths, requireData, type Scope } from '@/shared/api';

export type Vacancy = components['schemas']['Vacancy'];
export type VacancyPage = components['schemas']['VacancyPage'];
export type ApplicationEligibility = components['schemas']['ApplicationEligibility'];
export type VacancyFilters = NonNullable<paths['/api/v1/vacancies']['get']['parameters']['query']>;

export const vacanciesKey = (scope: Scope) => [...scope, 'vacancies'] as const;

export const vacancySearchQuery = (scope: Scope, filters: VacancyFilters) =>
  queryOptions({
    refetchInterval: 30_000,
    refetchIntervalInBackground: false,
    queryKey: [...vacanciesKey(scope), 'search', filters] as const,
    queryFn: async ({ signal }) =>
      requireData(await api.GET('/api/v1/vacancies', { params: { query: filters }, signal })),
    placeholderData: keepPreviousData,
  });

export const vacancyQuery = (scope: Scope, vacancyId: string) =>
  queryOptions({
    refetchInterval: 30_000,
    refetchIntervalInBackground: false,
    queryKey: [...vacanciesKey(scope), vacancyId] as const,
    queryFn: async ({ signal }) =>
      requireData(await api.GET('/api/v1/vacancies/{vacancyId}', { params: { path: { vacancyId } }, signal })),
  });

/** Why the server refuses an application, in the candidate's words, with the place to fix it. */
export const eligibilityView: Record<
  NonNullable<ApplicationEligibility['reason']>,
  {
    text: string;
    action: { label: string; to: '/candidate/onboarding' | '/candidate/assessments' | '/candidate/settings' } | null;
  }
> = {
  email: { text: 'Подтвердите почту — без неё отклик невозможен.', action: null },
  questionnaire: {
    text: 'Заполните анкету: компания увидит специализацию и опыт из неё.',
    action: { label: 'Заполнить анкету', to: '/candidate/onboarding' },
  },
  category: {
    text: 'Откликаться можно, когда у профиля есть категория: подключите GitHub или ФСП для автоматической оценки либо пройдите тест знаний.',
    action: { label: 'Открыть анкету', to: '/candidate/onboarding' },
  },
  processing_consent: {
    text: 'Для отклика нужно согласие на обработку данных.',
    action: { label: 'Открыть настройки', to: '/candidate/settings' },
  },
  application_exists: { text: 'Вы уже откликнулись на эту вакансию.', action: null },
};
