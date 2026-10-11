import catalog from '../../backend/crates/domain/data/catalog.json';
import type { components } from './generated/schema.d.ts';

type Meta = components['schemas']['Meta'];
type Grade = Meta['grades'][number]['id'];
type Specialization = Meta['specializations'][number]['id'];
type SkillId = Meta['skills'][number]['id'];

/** The catalog shared with the Rust domain (`backend/crates/domain/data/catalog.json`). */
export const CATALOG_VERSION = catalog.version;
export const GRADES = catalog.grades as Meta['grades'];
export const SPECIALIZATIONS: Meta['specializations'] = catalog.specializations.map(({ id, label, short, group }) => ({
  id: id as Specialization,
  label,
  short,
  group,
}));
export const SKILL_CATEGORIES = catalog.skillCategories as Meta['skillCategories'];
export const SKILLS = catalog.skills as Meta['skills'];

/** Grades in ascending order, for comparisons. */
export const GRADE_ORDER = GRADES.map((item) => item.id);

export const skillLabel = (id: string) => SKILLS.find((item) => item.id === id)?.label ?? id;
export const gradeLabel = (id: Grade) => GRADES.find((item) => item.id === id)?.label ?? id;
export const specializationShort = (id: Specialization) => SPECIALIZATIONS.find((item) => item.id === id)?.short ?? id;

/** The two topics a specialization's knowledge test covers, with labels. */
export function testTopics(specialization: Specialization): readonly [[SkillId, string], [SkillId, string]] {
  const entry = catalog.specializations.find((item) => item.id === specialization);
  const [first = '', second = ''] = entry?.testTopics ?? [];
  return [
    [first as SkillId, skillLabel(first)],
    [second as SkillId, skillLabel(second)],
  ];
}

export type { Grade, SkillId, Specialization };
