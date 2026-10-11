import { queryOptions } from '@tanstack/react-query';
import { api, type components, requireData, type Scope } from '@/shared/api';
import { gradeLabel, gradeRank } from '@/shared/lib';

export type SelfSkill = components['schemas']['SelfSkill'];
export type Skill = components['schemas']['Skill'];
export type SkillStatus = Skill['status'];
export type Evidence = components['schemas']['Evidence'];
export type Graph = components['schemas']['Graph'];
export type GapPlan = components['schemas']['GapPlan'];
type Grade = components['schemas']['Profile']['selectedGrade'];

export const skillsKey = (scope: Scope) => [...scope, 'skills'] as const;
export const graphKey = (scope: Scope) => [...scope, 'graph'] as const;

export const mySkillsQuery = (scope: Scope) =>
  queryOptions({
    queryKey: skillsKey(scope),
    queryFn: async ({ signal }) => requireData(await api.GET('/api/v1/me/skills', { signal })),
  });

export const myGraphQuery = (scope: Scope) =>
  queryOptions({
    queryKey: graphKey(scope),
    // Retrying a slow inference doubles the wait; the graph offers an explicit retry.
    retry: false,
    queryFn: async ({ signal }) => requireData(await api.GET('/api/v1/me/graph', { signal })),
  });

export const myGapPlanQuery = (scope: Scope, needId: string) =>
  queryOptions({
    queryKey: [...scope, 'gap-plan', needId] as const,
    queryFn: async ({ signal }) =>
      requireData(await api.GET('/api/v1/me/gap-plan', { params: { query: { needId } }, signal })),
  });

/** Names the level a claim covers, so "passed" is never ambiguous against the current target. */
export function levelContext(skill: Pick<Skill, 'status' | 'evidenceGrade'>, target: Grade | null) {
  if (skill.status === 'confirmed' && skill.evidenceGrade) {
    const confirmed = `Подтверждено для уровня «${gradeLabel[skill.evidenceGrade]}»`;
    if (target && gradeRank(target) > gradeRank(skill.evidenceGrade)) {
      return `${confirmed}; для уровня «${gradeLabel[target]}» нужна проверка`;
    }
    return confirmed;
  }
  if (target) return `Для уровня «${gradeLabel[target]}» нужна проверка`;
  return 'Уровень не подтверждён';
}
