import {
  type components,
  GRADE_IDS,
  GRADES,
  SKILL_CATEGORIES,
  SKILLS,
  SPECIALIZATION_GROUPS,
  SPECIALIZATION_IDS,
  SPECIALIZATIONS,
} from '@/shared/api';

type S = components['schemas'];

type Grade = S['Profile']['selectedGrade'];
type Specialization = S['Profile']['specialization'];

/** Grade labels from the shared catalog, e.g. «Мидл». */
export const gradeLabel = Object.fromEntries(GRADES.map((item) => [item.id, item.label])) as Record<Grade, string>;

/** Short specialization labels, e.g. «1С». */
export const specializationLabel = Object.fromEntries(SPECIALIZATIONS.map((item) => [item.id, item.short])) as Record<
  Specialization,
  string
>;

/** Full specialization names, e.g. «Разработка на 1С». */
export const specializationName = Object.fromEntries(SPECIALIZATIONS.map((item) => [item.id, item.label])) as Record<
  Specialization,
  string
>;

/** Person titles, e.g. «Разработчик 1С». */
export const roleTitle = Object.fromEntries(SPECIALIZATIONS.map((item) => [item.id, item.role])) as Record<
  Specialization,
  string
>;

/** The position of a grade in the catalog order, for comparisons. */
export const gradeRank = (grade: Grade) => GRADE_IDS.indexOf(grade);

export const industryLabel: Record<S['Profile']['industry'], string> = { it: 'Информационные технологии' };

export const workModeLabel: Record<S['Need']['workMode'], string> = {
  remote: 'Удалённо',
  hybrid: 'Гибрид',
  office: 'В офисе',
};

export const evidenceSourceLabel: Record<S['Evidence']['source'], string> = {
  github: 'GitHub',
  fsp: 'ФСП',
  assessment: 'Тест знаний',
};

export const criticalityLabel: Record<S['Requirement']['criticality'], string> = {
  required: 'Обязательно',
  desired: 'Желательно',
};

export const gradeOrder: Grade[] = [...GRADE_IDS];
export const specializationOrder: Specialization[] = [...SPECIALIZATION_IDS];
export const workModeOrder: S['Need']['workMode'][] = ['remote', 'hybrid', 'office'];

type SkillId = (typeof SKILLS)[number]['id'];

/** Catalog skills grouped by category for selects and pickers, alphabetical within a category. */
export const skillOptions: { value: SkillId; label: string; group: string }[] = SKILL_CATEGORIES.flatMap((category) =>
  SKILLS.filter((skill) => skill.category === category.id)
    .map((skill) => ({ value: skill.id, label: skill.label, group: category.label }))
    .sort((left, right) => left.label.localeCompare(right.label, 'ru')),
);

/** Specializations grouped by direction for selects. */
export const specializationOptions: { value: Specialization; label: string; group: string }[] =
  SPECIALIZATION_GROUPS.flatMap((group) =>
    SPECIALIZATIONS.filter((item) => item.group === group.id).map((item) => ({
      value: item.id,
      label: item.label,
      group: group.label,
    })),
  );

export const skillLabelOf = (id: string) => SKILLS.find((skill) => skill.id === id)?.label ?? id;
