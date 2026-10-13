import type { components } from '@/shared/api';
import { evidenceSourceLabel, gradeLabel } from '@/shared/lib';
import type { IconName } from '@/shared/ui';
import { StatusChip, type Tone } from '@/shared/ui';

type MatchStatus = components['schemas']['Match']['status'];
type RequirementStatus = components['schemas']['MatchRequirement']['status'];
type View = { label: string; tone: Tone; icon: IconName };

export const matchGroupView: Record<MatchStatus, View> = {
  meets_required: { label: 'Подходят по обязательным требованиям', tone: 'success', icon: 'checkCircle' },
  needs_verification: { label: 'Требуют дополнительной проверки', tone: 'warning', icon: 'alert' },
  missing_required: {
    label: 'Не хватает свидетельств по обязательным требованиям',
    tone: 'neutral',
    icon: 'minusCircle',
  },
};

const requirementView: Record<RequirementStatus, View> = {
  meets: { label: 'Подтверждено', tone: 'success', icon: 'checkCircle' },
  needs_verification: { label: 'Есть основания, нужна проверка', tone: 'warning', icon: 'alert' },
  not_evidenced: { label: 'Недостаточно данных', tone: 'neutral', icon: 'minusCircle' },
};

export function RequirementStatusChip({ status, plain }: { status: RequirementStatus; plain?: boolean }) {
  const view = requirementView[status];
  return (
    <StatusChip tone={view.tone} icon={view.icon} plain={plain ?? false}>
      {view.label}
    </StatusChip>
  );
}

const shapes = ['■', '▲', '◆', '●'];

/** A decorative shape derived from the alias number keeps rows distinguishable without personal initials. */
export function aliasShape(alias: string) {
  const digits = Number(alias.replace(/\D/g, '')) || 0;
  return shapes[digits % shapes.length];
}

/** Shared explanation from the server; neither audience recomputes evidence or rank. */
export function RequirementEvidence({ requirement }: { requirement: components['schemas']['MatchRequirement'] }) {
  return (
    <div className="stack small">
      <p>{requirement.reason}</p>
      <p className="muted">
        Требуется: от {gradeLabel[requirement.minimumGrade]}. Подтверждённый уровень:{' '}
        {requirement.evidenceGrade ? gradeLabel[requirement.evidenceGrade] : 'ещё не определён'}.
      </p>
      <p className="muted">
        Источники: {requirement.evidenceSources.map((source) => evidenceSourceLabel[source]).join(', ') || 'пока нет'}.{' '}
        Принимаются для этой вакансии:{' '}
        {requirement.acceptedSources.map((source) => evidenceSourceLabel[source]).join(', ') || 'нет среди имеющихся'}.
      </p>
    </div>
  );
}
