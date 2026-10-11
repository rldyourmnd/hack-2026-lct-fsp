import type { components } from '@/shared/api';
import { formatDate, gradeLabel, specializationLabel } from '@/shared/lib';
import { StatusChip } from '@/shared/ui';

type Grade = components['schemas']['Profile']['selectedGrade'];
type Specialization = components['schemas']['Profile']['specialization'];

type VerifiedProps = { grade: Grade | null; specialization?: Specialization | null; withSpecialization?: boolean };

/** The only badge that may state a confirmed grade; it is fed by server-verified fields. */
export function VerifiedGradeBadge({ grade, specialization, withSpecialization }: VerifiedProps) {
  if (!grade) return <StatusChip tone="neutral">Уровень ещё не подтверждён</StatusChip>;
  const prefix = withSpecialization && specialization ? `${specializationLabel[specialization]} · ` : '';
  return <StatusChip tone="success">{`${prefix}${gradeLabel[grade]} подтверждён`}</StatusChip>;
}

type CategorySource = NonNullable<components['schemas']['Category']['source']>;
type Confidence = NonNullable<components['schemas']['Category']['confidence']>;

export const categorySourceLabel: Record<CategorySource, string> = {
  assessment: 'подтверждено тестом',
  estimate: 'оценка по открытым данным',
};

export const estimateConfidenceLabel: Record<Confidence, string> = {
  low: 'низкая уверенность',
  medium: 'средняя уверенность',
  high: 'высокая уверенность',
};

type CategoryProps = {
  specialization: Specialization | null;
  grade: Grade | null;
  source: CategorySource | null;
  confidence: Confidence | null;
};

/**
 * The employer-visible category with where it comes from: a passed test, or an estimate from
 * public evidence with its confidence. Only a test result reads as «подтверждено».
 */
export function CategoryBadge({ specialization, grade, source, confidence }: CategoryProps) {
  if (!specialization || !grade || !source) return <StatusChip tone="neutral">Категория не присвоена</StatusChip>;
  const category = `${specializationLabel[specialization]} · ${gradeLabel[grade]}`;
  if (source === 'assessment')
    return (
      <StatusChip tone="success" icon="checkCircle">{`${category} · ${categorySourceLabel.assessment}`}</StatusChip>
    );
  return (
    <StatusChip tone="info" icon="chart">
      {`${category} · оценка${confidence ? `, ${estimateConfidenceLabel[confidence]}` : ''}`}
    </StatusChip>
  );
}

export function TargetGradeBadge({ grade }: { grade: Grade }) {
  return (
    <StatusChip tone="info" icon="chart">
      Цель: {gradeLabel[grade]}
    </StatusChip>
  );
}

export function qualificationText(specialization: Specialization | null, grade: Grade | null) {
  if (!specialization || !grade) return 'Категория не присвоена';
  return `${specializationLabel[specialization]} · ${gradeLabel[grade]}`;
}

type Freshness = components['schemas']['Freshness'];

/** How recent the last server-counted signal is: a verified grade or a short task the company did not reject. */
export function FreshnessChip({ freshness }: { freshness: Freshness }) {
  if (!freshness.lastSignalAt) {
    return (
      <StatusChip tone="neutral" icon="clock">
        Свежих подтверждений нет
      </StatusChip>
    );
  }
  const what = freshness.lastSignal === 'task' ? 'задание' : 'проверка';
  const when = formatDate(freshness.lastSignalAt);
  return freshness.fresh ? (
    <StatusChip tone="success" icon="activity">
      Профиль актуален · {what} {when}
    </StatusChip>
  ) : (
    <StatusChip tone="neutral" icon="clock">
      Давно не обновлялся · {what} {when}
    </StatusChip>
  );
}
