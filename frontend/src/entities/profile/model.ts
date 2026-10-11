import { queryOptions } from '@tanstack/react-query';
import { api, type components, requireData, type Scope } from '@/shared/api';

export type Profile = components['schemas']['Profile'];
export type ProfilePatch = components['schemas']['ProfilePatch'];
export type Onboarding = components['schemas']['Onboarding'];
export type Eligibility = components['schemas']['GradeEligibility'];

export const profileKey = (scope: Scope) => [...scope, 'profile'] as const;

export const profileQuery = (scope: Scope) =>
  queryOptions({
    queryKey: profileKey(scope),
    queryFn: async ({ signal }) => requireData(await api.GET('/api/v1/me/profile', { signal })),
  });

export async function updateProfile(patch: ProfilePatch) {
  return requireData(await api.PATCH('/api/v1/me/profile', { body: patch }));
}

/** `keep` answers only the fields the candidate has not edited; `replace` rebuilds every answer but the name. */
export type EstimateAnswers = 'keep' | 'replace';

export async function refreshEstimate(answers: EstimateAnswers = 'keep') {
  return requireData(await api.POST('/api/v1/me/profile/estimate', { params: { query: { answers } } }));
}

export type ProfileEstimate = components['schemas']['ProfileEstimate'];
export type AutoField = Profile['autoFields'][number];

export const blockerLabel: Record<Onboarding['blockers'][number], string> = {
  email: 'почта не подтверждена',
  questionnaire: 'анкета не заполнена',
  category: 'нет категории: подключите GitHub или ФСП либо пройдите тест',
  processing_consent: 'нет согласия на обработку данных',
  publication_consent: 'показ профиля работодателям выключен',
};

/** Marks a questionnaire field the system answered from evidence; the form explains the rule once. */
export const AUTO_FIELD_HINT = 'Заполнено автоматически.';

export const eligibilityReasonText: Record<NonNullable<Eligibility['selectedReason']>, string> = {
  cooldown: 'Подтверждённый грейд недавно менялся. Новую смену грейда можно начать после указанной даты.',
  onboarding_incomplete: 'Сначала подтвердите почту, заполните анкету и дайте согласие на обработку данных.',
  active_assessment: 'У вас есть незавершённая проверка. Продолжите её.',
  unsupported_scope:
    'Для выбранной специализации и грейда тест пока недоступен. Доступные направления указаны на странице «Проверки».',
  grade_downgrade: 'Проверка ниже подтверждённого грейда не проводится: подтверждённый грейд не понижается.',
};

/** Whole years in IT from the start year, as the server counts them. */
export const yearsInIt = (since: number, now = new Date()) => Math.max(0, now.getFullYear() - since);

/** The candidate's experience in words: from the start year when given, else the stated months. */
export function experienceLabel(profile: Pick<Profile, 'experienceSince' | 'experienceMonths'>): string | null {
  const plural = (value: number) =>
    value % 100 >= 11 && value % 100 <= 14
      ? 'лет'
      : value % 10 === 1
        ? 'год'
        : value % 10 >= 2 && value % 10 <= 4
          ? 'года'
          : 'лет';
  if (profile.experienceSince !== null) {
    const years = yearsInIt(profile.experienceSince);
    return `в IT с ${profile.experienceSince} года — ${years} ${plural(years)}`;
  }
  if (profile.experienceMonths === null) return null;
  const years = Math.floor(profile.experienceMonths / 12);
  const months = profile.experienceMonths % 12;
  return (
    [years ? `${years} ${plural(years)}` : '', months ? `${months} мес.` : ''].filter(Boolean).join(' ') || '0 мес.'
  );
}
