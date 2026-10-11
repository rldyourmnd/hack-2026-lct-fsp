import { useQuery, useSuspenseQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { type Assessment, AssessmentStatusChip, assessmentsQuery, isEditable } from '@/entities/assessment';
import { useMeta } from '@/entities/meta';
import { profileQuery } from '@/entities/profile';
import { useScope } from '@/entities/session';
import { SPECIALIZATIONS } from '@/shared/api';
import {
  formatDate,
  gradeLabel,
  pluralRu,
  skillLabelOf,
  specializationLabel,
  specializationName,
  useDocumentTitle,
} from '@/shared/lib';
import { Card, EmptyState, ErrorState, Icon, LoadingState, PageHeader, TableWrap } from '@/shared/ui';
import { AssessmentNextStep } from '@/widgets/assessment-next-step';
import { GraphInterview } from '@/widgets/graph-interview';

/** The two catalog topics each specialization's knowledge test covers. */
const assessmentTopics = Object.fromEntries(
  SPECIALIZATIONS.map((item) => [item.id, item.testTopics.map(skillLabelOf).join(' и ')]),
) as Record<keyof typeof specializationLabel, string>;

function categoryEffect(assessment: Assessment) {
  const result = assessment.result;
  if (!result) return 'Пока без изменений';
  if (result.outcome === 'service_error') return 'Без изменений, штрафа и ожидания нет';
  if (result.outcome === 'failed') {
    return result.previousGrade ? `Грейд «${gradeLabel[result.previousGrade]}» сохранён` : 'Категория не присвоена';
  }
  if (!result.gradeChanged) return 'Грейд подтверждён повторно';
  return result.previousGrade && result.confirmedGrade
    ? `${gradeLabel[result.previousGrade]} → ${gradeLabel[result.confirmedGrade]}`
    : `Подтверждён ${result.confirmedGrade ? gradeLabel[result.confirmedGrade] : 'грейд'}`;
}

function History() {
  const scope = useScope();
  const assessments = useQuery(assessmentsQuery(scope));
  return (
    <Card
      title="История попыток"
      subtitle="Результат каждой попытки хранится неизменным, даже если профиль потом меняется."
    >
      {assessments.isPending && <LoadingState />}
      {assessments.isError && <ErrorState error={assessments.error} onRetry={() => assessments.refetch()} />}
      {assessments.data?.items.length === 0 && (
        <EmptyState icon="shield" title="Проверок ещё не было">
          Первая успешная проверка присвоит категорию, которую видят работодатели.
        </EmptyState>
      )}
      {assessments.data && assessments.data.items.length > 0 && (
        <TableWrap label="История проверок">
          <table className="table">
            <thead>
              <tr>
                <th scope="col">Дата</th>
                <th scope="col">Цель</th>
                <th scope="col">Статус</th>
                <th scope="col">Влияние на категорию</th>
                <th scope="col">
                  <span className="visually-hidden">Действие</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {assessments.data.items.map((item) => (
                <tr key={item.id}>
                  <td>{formatDate(item.createdAt)}</td>
                  <td>
                    {specializationLabel[item.specialization]} · {gradeLabel[item.grade]}
                    <div className="small muted">
                      {item.task.title}, вариант {item.task.variantId}
                    </div>
                  </td>
                  <td>
                    <AssessmentStatusChip status={item.status} />
                  </td>
                  <td>{categoryEffect(item)}</td>
                  <td>
                    {isEditable(item) ? (
                      <Link to="/candidate/assessments/$assessmentId/workspace" params={{ assessmentId: item.id }}>
                        Продолжить
                      </Link>
                    ) : (
                      <Link to="/candidate/assessments/$assessmentId/result" params={{ assessmentId: item.id }}>
                        Подробнее
                      </Link>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </TableWrap>
      )}
    </Card>
  );
}

export function CandidateAssessmentsPage() {
  useDocumentTitle('Проверки');
  const scope = useScope();
  const meta = useMeta();
  const { data: profile } = useSuspenseQuery(profileQuery(scope));
  const policy = meta.assessmentPolicy;
  const everyScope =
    policy.supportedScopes.length === SPECIALIZATIONS.length &&
    policy.supportedScopes.every((item) => item.grades.length === Object.keys(gradeLabel).length);
  const scopes = policy.supportedScopes
    .map((item) => {
      const grades = item.grades.map((grade) => gradeLabel[grade]).join(', ');
      return `${specializationLabel[item.specialization]} (${assessmentTopics[item.specialization]}): ${grades}`;
    })
    .join('; ');
  return (
    <>
      <PageHeader title="Проверки" subtitle="Тест знаний и адаптивное собеседование по графу компетенций" />
      <GraphInterview available={meta.capabilities.graphInterviews} />
      <div className="split">
        <Card title="Как устроена проверка">
          <div className="stack">
            <ol className="stack" style={{ margin: 0, paddingLeft: '1.25rem' }}>
              <li>
                <strong>Вопросы.</strong> 10 вопросов: по 5 на каждую из двух тем направления. В каждом один верный
                ответ.
              </li>
              <li>
                <strong>Черновик.</strong> Выбранные ответы автоматически сохраняются на сервере.
              </li>
              <li>
                <strong>Результат.</strong> Зачёт — от 8 из 10 и от 3 из 5 по каждой теме. После отправки доступен
                разбор.
              </li>
            </ol>
            <ul className="stack small" style={{ margin: 0, paddingLeft: '1.25rem' }}>
              {policy.aiAllowed && (
                <li>
                  <Icon name="sparkles" size={16} /> ИИ разрешён на всех этапах
                  {policy.externalAiAllowed ? ', включая внешние инструменты' : ''}.
                </li>
              )}
              <li>Неудачная попытка не снижает подтверждённый грейд и не добавляет ожидания.</li>
              <li>Сбой сервиса оценивания не считается неудачей: можно начать новую попытку без штрафа.</li>
              <li>
                Между успешными сменами грейда — {policy.successfulGradeChangeCooldownDays}{' '}
                {pluralRu(policy.successfulGradeChangeCooldownDays, 'день', 'дня', 'дней')}. Повтор на том же грейде
                ожидания не требует.
              </li>
              <li>Проверка ниже уже подтверждённого грейда не проводится.</li>
              <li>
                {everyScope
                  ? `Тест есть для всех ${SPECIALIZATIONS.length} направлений и всех грейдов — от стажёра до лида.`
                  : `Доступные направления: ${scopes}.`}
              </li>
            </ul>
            <details>
              <summary className="small">Темы тестов по направлениям</summary>
              <ul className="small" style={{ margin: '0.5rem 0 0', paddingLeft: '1.25rem' }}>
                {policy.supportedScopes.map((item) => (
                  <li key={item.specialization}>
                    {specializationName[item.specialization]}: {assessmentTopics[item.specialization]}
                  </li>
                ))}
              </ul>
            </details>
            <p className="small muted">
              Уровень по этому тесту отражает знания выбранных тем, а не проверенный опыт разработки. Банк вопросов
              открыт для изучения; ИИ и справочники разрешены.
            </p>
          </div>
        </Card>
        <AssessmentNextStep profile={profile} />
      </div>
      <div style={{ marginTop: '1.5rem' }}>
        <History />
      </div>
    </>
  );
}
