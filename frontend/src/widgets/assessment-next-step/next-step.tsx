import { useQuery } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { assessmentsQuery, isEditable, isEvaluating } from '@/entities/assessment';
import { useMeta } from '@/entities/meta';
import { eligibilityReasonText, type Profile } from '@/entities/profile';
import { useScope } from '@/entities/session';
import { useStartAssessment } from '@/features/start-assessment';
import { describeError } from '@/shared/api';
import { daysUntil, formatDate, gradeLabel, pluralRu, specializationLabel } from '@/shared/lib';
import { Button, ButtonLink, Callout, Card, ErrorState, Icon, LoadingState } from '@/shared/ui';

/** The primary assessment action, driven only by the server's eligibility and attempt state. */
export function AssessmentNextStep({ profile }: { profile: Profile }) {
  const scope = useScope();
  const meta = useMeta();
  const assessments = useQuery(assessmentsQuery(scope));
  const start = useStartAssessment();
  const eligibility = profile.assessmentEligibility;
  const target = `${specializationLabel[eligibility.selectedSpecialization]} · ${gradeLabel[eligibility.selectedGrade]}`;

  if (assessments.isPending) {
    return (
      <Card title="Следующий шаг" tinted>
        <LoadingState lines={2} />
      </Card>
    );
  }
  if (assessments.isError) {
    return (
      <Card title="Следующий шаг" tinted>
        <ErrorState error={assessments.error} onRetry={() => assessments.refetch()} retrying={assessments.isFetching} />
      </Card>
    );
  }

  const active = assessments.data.items.find((item) => isEditable(item) || isEvaluating(item));
  const growth = (profile.estimate?.growth ?? []).filter((step) => step.id !== 'assessment');
  let body: ReactNode;

  if (!profile.onboarding.questionnaireComplete || !profile.consents.processing) {
    body = (
      <>
        <h3 style={{ margin: 0 }}>Соберите профиль</h3>
        <p className="muted">
          Дайте согласие на обработку и подключите GitHub или ФСП — анкета заполнится автоматически. Её можно заполнить
          и вручную.
        </p>
        <ButtonLink to="/candidate/onboarding" variant="primary" iconAfter="arrowRight">
          Продолжить
        </ButtonLink>
      </>
    );
  } else if (active && isEditable(active)) {
    body = (
      <>
        <h3 style={{ margin: 0 }}>Продолжите проверку</h3>
        <p className="muted">
          {specializationLabel[active.specialization]} · {gradeLabel[active.grade]}. Черновик сохранён на сервере.
        </p>
        <ButtonLink
          to="/candidate/assessments/$assessmentId/workspace"
          params={{ assessmentId: active.id }}
          variant="primary"
          iconAfter="arrowRight"
        >
          Продолжить проверку
        </ButtonLink>
      </>
    );
  } else if (active) {
    body = (
      <>
        <h3 style={{ margin: 0 }}>Решение проверяется</h3>
        <p className="muted">Текущий подтверждённый грейд сохраняется до итогового результата.</p>
        <ButtonLink to="/candidate/assessments/$assessmentId/result" params={{ assessmentId: active.id }}>
          Открыть статус
        </ButtonLink>
      </>
    );
  } else if (profile.category.source === 'estimate' && growth.length > 0) {
    // An estimated category grows with evidence first; the test stays an option, not a demand.
    body = (
      <>
        <h3 style={{ margin: 0 }}>Как повысить грейд</h3>
        <ul className="stack" style={{ margin: 0, paddingLeft: '1.25rem' }}>
          {growth.slice(0, 3).map((step) => (
            <li key={step.id}>
              <strong>{step.title}</strong>
              {step.points > 0 && ` — до +${step.points}`}
              <div className="small muted">{step.action}</div>
            </li>
          ))}
        </ul>
        {eligibility.canStartSelected && (
          <Button
            size="sm"
            iconAfter="arrowRight"
            pending={start.isPending}
            pendingLabel="Создаём проверку…"
            onClick={() =>
              start.mutate({ specialization: eligibility.selectedSpecialization, grade: eligibility.selectedGrade })
            }
          >
            Пройти тест — по желанию
          </Button>
        )}
        {start.error && <Callout tone="danger">{describeError(start.error).message}</Callout>}
      </>
    );
  } else if (eligibility.canStartSelected) {
    body = (
      <>
        <h3 style={{ margin: 0 }}>
          {profile.verifiedGrade
            ? `Подтвердить ${gradeLabel[eligibility.selectedGrade]}`
            : profile.category.source === 'estimate'
              ? 'Подтвердить категорию тестом'
              : 'Подтвердить уровень'}
        </h3>
        <p className="muted">
          {target}. Тест знаний с выбором ответов, ИИ разрешён.
          {profile.category.source === 'estimate' && ' Подтверждённые кандидаты стоят выше в выдаче.'}
        </p>
        <Button
          variant="primary"
          iconAfter="arrowRight"
          pending={start.isPending}
          pendingLabel="Создаём проверку…"
          onClick={() =>
            start.mutate({ specialization: eligibility.selectedSpecialization, grade: eligibility.selectedGrade })
          }
        >
          Начать проверку
        </Button>
        {profile.verifiedGrade && (
          <p className="small muted" style={{ display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
            <Icon name="info" size={16} />
            Текущий грейд сохранится при любом результате
          </p>
        )}
        {start.error && <Callout tone="danger">{describeError(start.error).message}</Callout>}
      </>
    );
  } else {
    const days = eligibility.nextEligibleAt ? daysUntil(meta.serverTime, eligibility.nextEligibleAt) : null;
    body = (
      <>
        <h3 style={{ margin: 0 }}>
          {eligibility.selectedReason === 'cooldown' ? 'Смена грейда пока недоступна' : 'Проверка недоступна'}
        </h3>
        <p className="muted">
          {eligibility.selectedReason ? eligibilityReasonText[eligibility.selectedReason] : 'Проверка пока недоступна.'}
        </p>
        {eligibility.selectedReason === 'cooldown' && eligibility.nextEligibleAt && (
          <p>
            Доступно с <strong>{formatDate(eligibility.nextEligibleAt)}</strong>
            {days !== null && days > 0 && ` (через ${days} ${pluralRu(days, 'день', 'дня', 'дней')})`}. Интервал между
            успешными сменами грейда — {eligibility.cooldownDays}{' '}
            {pluralRu(eligibility.cooldownDays, 'день', 'дня', 'дней')}.
          </p>
        )}
        <ButtonLink to="/candidate/profile">Изменить цель проверки</ButtonLink>
      </>
    );
  }

  return (
    <Card title="Следующий шаг" tinted>
      <div className="stack">{body}</div>
    </Card>
  );
}
