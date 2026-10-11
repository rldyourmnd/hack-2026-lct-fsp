import { readFileSync } from 'node:fs';
import type { Candidate } from './data.ts';
import type { components } from './generated/schema.d.ts';

type S = components['schemas'];
type Estimate = S['ProfileEstimate'];
export type AutoField = S['Profile']['autoFields'][number];
export type Category = {
  specialization: NonNullable<S['Category']['specialization']>;
  grade: NonNullable<S['Category']['grade']>;
  source: NonNullable<S['Category']['source']>;
  confidence: S['Category']['confidence'];
};

/**
 * Behavior fixture for the development API; the Rust backend computes estimates with `estimate-v6`.
 *
 * The mock's evidence is fixed: one GitHub fixture report and one fictional published FSP result.
 * The Rust rules wrote their estimates to `contracts/fixtures/profile-estimates.json`, so the mock
 * applies those instead of reimplementing the rules. It ignores experience the candidate typed.
 */
const ESTIMATES = JSON.parse(
  readFileSync(new URL('../../contracts/fixtures/profile-estimates.json', import.meta.url), 'utf8'),
) as Record<'none' | 'github' | 'fsp' | 'githubFsp', Estimate>;

/** The name a new account starts with; an estimate may replace it. */
export const DEFAULT_CANDIDATE_NAME = 'Новый кандидат';

/** The estimate from the candidate's finished sources. */
export function sourceEstimate(candidate: Candidate, clock: string): Estimate {
  const github = candidate.sources.some((source) => source.provider === 'github' && source.github?.report);
  const fsp = candidate.sources.some(
    (source) => source.provider === 'fsp' && (source.fspResults ?? []).some((result) => result.status === 'published'),
  );
  const key = github ? (fsp ? 'githubFsp' : 'github') : fsp ? 'fsp' : 'none';
  return { ...structuredClone(ESTIMATES[key]), computedAt: clock };
}

export function questionnaireComplete(candidate: Candidate): boolean {
  return Boolean(
    candidate.name.trim() && candidate.stack.length && candidate.teamRole.trim() && candidate.experience.trim(),
  );
}

/** Questionnaire fields an estimate may answer, as the Rust `AUTO_FIELDS`. */
export const AUTO_FIELDS: readonly AutoField[] = [
  'name',
  'specialization',
  'selectedGrade',
  'stack',
  'experience',
  'teamRole',
  'softSkills',
];

/** Mirrors `CandidateProfile::release_answers`: every answer but an edited name goes back to the evidence. */
export function releaseAnswers(candidate: Candidate): void {
  const name = candidate.autoFields.includes('name');
  candidate.autoFields = AUTO_FIELDS.filter((field) => field !== 'name' || name);
}

/** Mirrors `CandidateProfile::rebuild_from_estimate`: unanswered released fields return to the candidate. */
export function rebuildFromEstimate(candidate: Candidate, estimate: Estimate): void {
  const filled = applyEstimate(candidate, estimate);
  candidate.autoFields = candidate.autoFields.filter((field) => filled.includes(field));
}

/**
 * Mirrors the Rust `CandidateProfile::apply_estimate`: answers empty fields, fields an earlier
 * estimate filled, and the specialization and target grade while the questionnaire is incomplete.
 * Edited answers and a test-confirmed category are kept. Returns the fields it answered.
 */
export function applyEstimate(candidate: Candidate, estimate: Estimate): AutoField[] {
  const incomplete = !questionnaireComplete(candidate);
  const confirmed = candidate.verifiedGrade !== null;
  const auto = (field: AutoField) => candidate.autoFields.includes(field);
  const filled: AutoField[] = [];
  const { answers } = estimate;
  const name = answers.name?.trim();
  if (name && (auto('name') || !candidate.name.trim() || candidate.name === DEFAULT_CANDIDATE_NAME)) {
    candidate.name = name.slice(0, 120);
    filled.push('name');
  }
  if (estimate.specialization && !confirmed && (auto('specialization') || incomplete)) {
    candidate.specialization = estimate.specialization;
    filled.push('specialization');
  }
  if (estimate.grade && !confirmed && (auto('selectedGrade') || incomplete)) {
    candidate.selectedGrade = estimate.grade;
    filled.push('selectedGrade');
  }
  if (answers.stack.length && (auto('stack') || !candidate.stack.length)) {
    candidate.stack = [...answers.stack];
    filled.push('stack');
  }
  // Earlier estimates wrote public tenure into the answer; it was never the candidate's.
  if (auto('experienceMonths')) {
    candidate.experienceMonths = null;
    candidate.autoFields = candidate.autoFields.filter((field) => field !== 'experienceMonths');
  }
  if (answers.experience.trim() && (auto('experience') || !candidate.experience.trim())) {
    candidate.experience = answers.experience.slice(0, 4000);
    filled.push('experience');
  }
  if (answers.teamRole.trim() && (auto('teamRole') || !candidate.teamRole.trim())) {
    candidate.teamRole = answers.teamRole.slice(0, 120);
    filled.push('teamRole');
  }
  if (answers.softSkills.length && (auto('softSkills') || !candidate.softSkills.length)) {
    candidate.softSkills = answers.softSkills.slice(0, 12).map((skill) => skill.slice(0, 100));
    filled.push('softSkills');
  }
  for (const field of filled) if (!auto(field)) candidate.autoFields.push(field);
  candidate.estimate = estimate;
  return filled;
}

/** A passed test, otherwise an estimate with at least medium confidence. */
export function categoryOf(candidate: Candidate): Category | null {
  if (candidate.verifiedSpecialization && candidate.verifiedGrade)
    return {
      specialization: candidate.verifiedSpecialization,
      grade: candidate.verifiedGrade,
      source: 'assessment',
      confidence: null,
    };
  const estimate = candidate.estimate;
  if (!estimate?.specialization || !estimate.grade || estimate.confidence === 'low') return null;
  // The candidate may choose among the specializations the evidence supports.
  const chosen = estimate.specializations.some((share) => share.id === candidate.specialization);
  return {
    specialization: chosen ? candidate.specialization : estimate.specialization,
    grade: estimate.grade,
    source: 'estimate',
    confidence: estimate.confidence,
  };
}

/** Test = 3, high-confidence estimate = 2, medium = 1, as the Rust `Category::strength`. */
export function categoryStrength(category: Category | null): number {
  if (!category) return 0;
  if (category.source === 'assessment') return 3;
  return category.confidence === 'high' ? 2 : 1;
}
