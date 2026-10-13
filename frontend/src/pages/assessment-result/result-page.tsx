import { useQuery, useQueryClient, useSuspenseQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { useEffect } from 'react';
import {
  type Assessment,
  assessmentQuery,
  assessmentsKey,
  CheckStatusChip,
  isEvaluating,
  RubricOutcomeChip,
} from '@/entities/assessment';
import { eligibilityReasonText, profileKey, profileQuery } from '@/entities/profile';
import { useScope } from '@/entities/session';
import { graphKey, mySkillsQuery, SkillStatusChip, skillsKey } from '@/entities/skill';
import { vacanciesKey } from '@/entities/vacancy';
import { ExportProfileButton } from '@/features/profile-pdf';
import { useStartAssessment } from '@/features/start-assessment';
import { describeError } from '@/shared/api';
import { formatDate, gradeLabel, initials, specializationLabel, useDocumentTitle } from '@/shared/lib';
import { Avatar, Button, ButtonLink, Callout, Card, Icon, InlineStatus, StatusChip } from '@/shared/ui';
import styles from './result-page.module.css';

function headline(assessment: Assessment) {
  const result = assessment.result;
  const target = gradeLabel[assessment.grade];
  if (!result)
    return {
      icon: 'clock' as const,
      tone: 'info',
      title: assessment.task.execution === 'questionnaire' ? 'Ответы проверяются' : 'Архивная демонстрационная задача',
    };
  if (result.outcome === 'service_error')
    return { icon: 'alert' as const, tone: 'warning', title: 'Оценка не получена' };
  if (result.outcome === 'failed') {
    return {
      icon: 'xCircle' as const,
      tone: 'danger',
      title: result.previousGrade ? `${target} пока не подтверждён` : 'Проверка не пройдена',
    };
  }
  return {
    icon: 'checkCircle' as const,
    tone: 'success',
    title: result.gradeChanged ? `${target} подтверждён` : `${target} подтверждён повторно`,
  };
}

function NextActions({ assessment }: { assessment: Assessment }) {
  const scope = useScope();
  const profile = useQuery(profileQuery(scope));
  const start = useStartAssessment();
  const eligibility = profile.data?.assessmentEligibility;
  if (!eligibility) return null;
  return (
    <Card title={assessment.result?.outcome === 'service_error' ? 'Что дальше' : 'Как продолжить'}>
      <div className="stack">
        {assessment.result?.outcome === 'service_error' ? (
          <p>Сбой сервиса не считается попыткой с отрицательным результатом. Можно начать новую попытку сразу.</p>
        ) : assessment.result?.previousGrade ? (
          <p>
            Подтверждённый грейд «{gradeLabel[assessment.result.previousGrade]}» остаётся в профиле. Можно сразу
            повторить тест или выбрать другую цель.
          </p>
        ) : (
          <p>Категория пока не присвоена. Можно повторить проверку или выбрать более подходящий грейд в профиле.</p>
        )}
        <div className="row">
          {eligibility.canStartSelected ? (
            <Button
              variant="primary"
              icon="refresh"
              pending={start.isPending}
              onClick={() =>
                start.mutate({ specialization: eligibility.selectedSpecialization, grade: eligibility.selectedGrade })
              }
            >
              Начать новую попытку · {gradeLabel[eligibility.selectedGrade]}
            </Button>
          ) : (
            <p className="small muted">
              {eligibility.selectedReason
                ? eligibilityReasonText[eligibility.selectedReason]
                : 'Новая попытка пока недоступна.'}
            </p>
          )}
          <ButtonLink to="/candidate/profile">Выбрать другую цель</ButtonLink>
        </div>
        {start.error && <Callout tone="danger">{describeError(start.error).message}</Callout>}
      </div>
    </Card>
  );
}

function EvidenceUpdated({ assessment }: { assessment: Assessment }) {
  const scope = useScope();
  const skills = useQuery(mySkillsQuery(scope));
  if (!skills.data) return null;
  const confirmed = skills.data.items.filter((skill) =>
    skill.evidence.some((evidence) => evidence.reference === assessment.id),
  );
  const gap = skills.data.items.find((skill) => skill.status === 'insufficient');
  return (
    <>
      <Card title="Свидетельства обновлены">
        {confirmed.length ? (
          <div className="stack">
            <p className="muted">
              Знания по темам этого теста подтверждены на уровне «{gradeLabel[assessment.grade]}»:
            </p>
            <div className="row">
              {confirmed.map((skill) => (
                <StatusChip key={skill.skillId} tone="success">
                  {skill.label}
                </StatusChip>
              ))}
            </div>
            <p className="small muted">
              Результат относится к вопросам этого теста, не охватывает все темы технологий и не подтверждает
              практический опыт. Остальные навыки не изменились.
            </p>
          </div>
        ) : (
          <p className="muted">Результат сохранён в истории. Актуальные свидетельства доступны на странице навыков.</p>
        )}
      </Card>
      {gap && (
        <Card title="Следующий навык" tinted>
          <div className="stack">
            <div className="row">
              <strong style={{ fontSize: '1.25rem' }}>{gap.label}</strong>
              <SkillStatusChip status={gap.status} />
            </div>
            <p className="small muted">
              Свидетельств пока недостаточно. Это пробел в данных, а не отрицательная оценка.
            </p>
            <ButtonLink
              to="/candidate/skills"
              search={{ node: `skill-${gap.skillId}` }}
              variant="primary"
              iconAfter="arrowRight"
            >
              Открыть навык
            </ButtonLink>
          </div>
        </Card>
      )}
    </>
  );
}

export function AssessmentResultPage({ assessmentId }: { assessmentId: string }) {
  useDocumentTitle('Результат проверки');
  const scope = useScope();
  const queryClient = useQueryClient();
  const { data: assessment } = useSuspenseQuery({
    ...assessmentQuery(scope, assessmentId),
    refetchInterval: (query) => (query.state.data && isEvaluating(query.state.data) ? 600 : false),
  });
  const profile = useQuery(profileQuery(scope));
  const terminal = !isEvaluating(assessment) && assessment.result !== null;

  useEffect(() => {
    if (!terminal) return;
    // The final outcome can change category and evidence; refresh everything derived from them.
    for (const queryKey of [
      profileKey(scope),
      skillsKey(scope),
      graphKey(scope),
      assessmentsKey(scope),
      vacanciesKey(scope),
    ]) {
      void queryClient.invalidateQueries({ queryKey });
    }
  }, [terminal, queryClient, scope]);

  const result = assessment.result;
  const head = headline(assessment);
  const profileUpdated =
    result?.outcome === 'passed' &&
    profile.data?.verifiedGrade === result.confirmedGrade &&
    profile.data?.verifiedSpecialization === assessment.specialization;

  if (!result && assessment.task.execution !== 'questionnaire') {
    return (
      <Card title="Архивная демонстрационная задача">
        <div className="stack">
          <p>Эта попытка больше не оценивается. Сохранённое решение доступно для просмотра.</p>
          <ButtonLink to="/candidate/assessments/$assessmentId/workspace" params={{ assessmentId }}>
            Открыть старое решение
          </ButtonLink>
          <ButtonLink to="/candidate/assessments">Перейти к тестам знаний</ButtonLink>
        </div>
      </Card>
    );
  }
  return (
    <>
      <header className={styles.header}>
        <span className={`${styles.badge} ${styles[head.tone]}`}>
          <Icon name={head.icon} size={30} />
        </span>
        <div>
          <h1 className={styles.title}>{head.title}</h1>
          <p className="muted" style={{ fontSize: '1.125rem' }}>
            {specializationLabel[assessment.specialization]} ·{' '}
            {assessment.task.execution === 'questionnaire' ? 'тест знаний' : 'архивная демонстрационная проверка'}{' '}
            {result ? `завершён ${formatDate(assessment.updatedAt)}` : 'ещё оценивается'}
          </p>
        </div>
      </header>

      {!result && (
        <Card>
          <div className="stack">
            <InlineStatus>Решение проверяется. Страницу можно закрыть — результат сохранится в истории.</InlineStatus>
            <p className="small muted">Текущий подтверждённый грейд сохраняется до итогового результата.</p>
          </div>
        </Card>
      )}

      {result && (
        <div className="stack-lg">
          <Card className={result.outcome === 'passed' ? styles.successCard : undefined}>
            <div className={styles.summary}>
              {profile.data && (
                <div className="row">
                  <Avatar kind="initials" text={initials(profile.data.name)} size="md" />
                  <div>
                    <strong>{profile.data.name}</strong>
                    <div className="small muted">{profile.data.alias}</div>
                  </div>
                </div>
              )}
              <div className={styles.change}>
                <span className={styles.grade}>
                  Было: <strong>{result.previousGrade ? gradeLabel[result.previousGrade] : 'не подтверждён'}</strong>
                </span>
                <Icon name="arrowRight" />
                <span className={styles.grade}>
                  Теперь:{' '}
                  <strong>{result.confirmedGrade ? gradeLabel[result.confirmedGrade] : 'не подтверждён'}</strong>
                </span>
              </div>
              <span className="row">
                {result.outcome === 'passed' ? (
                  profileUpdated ? (
                    <StatusChip tone="success">Профиль обновлён автоматически</StatusChip>
                  ) : (
                    <InlineStatus>Обновляем профиль…</InlineStatus>
                  )
                ) : (
                  <StatusChip tone="neutral" icon="lock">
                    Грейд не изменился
                  </StatusChip>
                )}
              </span>
            </div>
            <p style={{ marginTop: '1rem' }}>{result.feedback}</p>
            {result.outcome === 'service_error' && (
              <p className="small muted">
                Штрафа и дополнительного ожидания нет; отрицательные оценки по критериям не выставлялись.
              </p>
            )}
            {result.nextEligibleAt && result.outcome === 'passed' && result.gradeChanged && (
              <p className="small muted">Следующая смена грейда — не раньше {formatDate(result.nextEligibleAt)}</p>
            )}
          </Card>

          <div className="split">
            <div className="stack-lg">
              {result.rubric.length > 0 && (
                <Card title="Результаты по критериям">
                  <ul className="stack" style={{ listStyle: 'none', margin: 0, padding: 0 }}>
                    {result.rubric.map((item) => (
                      <li key={item.criterionId} className="stack" style={{ gap: '0.5rem' }}>
                        <div className="row">
                          <strong>{item.label}</strong>
                          <RubricOutcomeChip outcome={item.outcome} />
                        </div>
                        <p className="small muted">{item.feedback}</p>
                      </li>
                    ))}
                  </ul>
                </Card>
              )}
              {result.checks.length > 0 && (
                <Card title="Разбор ответов">
                  <ul className={styles.checks}>
                    {result.checks.map((check) => (
                      <li key={check.id}>
                        <div className="stack" style={{ gap: '0.5rem', minWidth: 0, overflowWrap: 'anywhere' }}>
                          <strong>{check.label}</strong>
                          <p className="small">
                            Ваш ответ:{' '}
                            {assessment.task.questions
                              ?.find((q) => q.id === check.id)
                              ?.options.find(
                                (o) =>
                                  o.id ===
                                  assessment.answer.responses?.find((r) => r.questionId === check.id)?.optionId,
                              )?.label ?? 'Архивный ответ'}
                          </p>
                          <p className="small muted">{check.message}</p>
                        </div>
                        <CheckStatusChip status={check.status} />
                      </li>
                    ))}
                  </ul>
                  <p className="small muted" style={{ marginTop: '0.75rem' }}>
                    Применены критерии теста «{assessment.task.family}», версия {assessment.task.version}.
                    {result.simulation && ' Результаты симулированы для демонстрации.'}
                  </p>
                </Card>
              )}
            </div>
            <div className="stack-lg">
              {result.outcome === 'passed' ? (
                assessment.task.execution === 'questionnaire' ? (
                  <EvidenceUpdated assessment={assessment} />
                ) : (
                  <Callout tone="neutral">
                    Архивный результат демонстрационной проверки. Знания по новому тесту здесь не оценивались.
                  </Callout>
                )
              ) : (
                <NextActions assessment={assessment} />
              )}
            </div>
          </div>
        </div>
      )}

      <div className={styles.actions}>
        <ButtonLink to="/candidate/overview" variant="primary" iconAfter="arrowRight">
          Открыть профиль
        </ButtonLink>
        <ExportProfileButton />
        <Link to="/candidate/assessments">История попыток →</Link>
        <span className="small muted" style={{ marginLeft: 'auto' }}>
          <Icon name="info" size={16} /> ИИ был разрешён на всех этапах
        </span>
      </div>
    </>
  );
}
