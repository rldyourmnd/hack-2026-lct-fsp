import { useSuspenseQuery } from '@tanstack/react-query';
import { skillLabel, useMeta } from '@/entities/meta';
import { FreshnessChip } from '@/entities/qualification';
import { useScope } from '@/entities/session';
import { type MyTasks, myTasksQuery, type TaskResponse, TaskResponseStatusChip, taskModeLabel } from '@/entities/task';
import { TaskResponseForm } from '@/features/respond-task';
import { formatDate, formatDateTime, gradeLabel, useDocumentTitle } from '@/shared/lib';
import { ButtonLink, Callout, Card, EmptyState, Icon, PageHeader, Tag } from '@/shared/ui';

const reasonView: Record<
  NonNullable<MyTasks['reason']>,
  { text: string; action: { label: string; to: '/candidate/onboarding' | '/candidate/settings' } | null }
> = {
  category: {
    text: 'Задания предлагаются, когда у профиля есть категория: компании дают задачи под её уровень. Подключите GitHub или ФСП либо пройдите тест.',
    action: { label: 'Открыть анкету', to: '/candidate/onboarding' },
  },
  processing_consent: {
    text: 'Для заданий нужно согласие на обработку данных.',
    action: { label: 'Открыть настройки', to: '/candidate/settings' },
  },
  no_tasks: {
    text: 'Подходящих заданий на этой неделе нет: компании ещё не добавили задачи по вашим навыкам и уровню.',
    action: null,
  },
};

function ResponseView({ response }: { response: TaskResponse }) {
  return (
    <div className="stack">
      <div className="row" style={{ justifyContent: 'space-between' }}>
        <strong>{taskModeLabel[response.mode]}</strong>
        <TaskResponseStatusChip status={response.status} />
      </div>
      <p style={{ whiteSpace: 'pre-wrap', margin: 0 }}>{response.answer}</p>
      <span className="small muted">Отправлено {formatDateTime(response.submittedAt)}</span>
      {response.feedback && (
        <Callout tone={response.status === 'accepted' ? 'success' : 'warning'} title="Комментарий компании">
          {response.feedback}
        </Callout>
      )}
    </div>
  );
}

export function CandidateTasksPage() {
  useDocumentTitle('Задания');
  const scope = useScope();
  const meta = useMeta();
  const { data } = useSuspenseQuery({ ...myTasksQuery(scope), refetchOnWindowFocus: 'always' });
  const { current, currentResponse, reason, period, freshness } = data;
  const history = data.history.filter((item) => item.id !== currentResponse?.id);
  const blocker = reason ? reasonView[reason] : null;

  return (
    <>
      <PageHeader
        title="Задания"
        subtitle="Раз в неделю платформа предлагает короткую задачу от компании по вашим навыкам. Ответ поддерживает профиль актуальным и даёт работодателям свежий сигнал."
      />
      <div className="split">
        <div className="stack-lg">
          {current ? (
            <Card title="Задание недели" subtitle={`До ${formatDate(period.to)} · от «${current.company.displayName}»`}>
              <div className="stack-lg">
                <div className="stack" style={{ gap: '0.5rem' }}>
                  <h3 style={{ margin: 0, fontSize: '1.25rem' }}>{current.title}</h3>
                  <div className="row">
                    <Tag>{skillLabel(meta, current.skillId)}</Tag>
                    <Tag>от {gradeLabel[current.grade]}</Tag>
                    <Tag>
                      <Icon name="clock" size={16} /> около {current.estimateMinutes} мин
                    </Tag>
                  </div>
                </div>
                <p style={{ whiteSpace: 'pre-wrap', margin: 0 }}>{current.prompt}</p>
                <hr className="divider" />
                {currentResponse ? <ResponseView response={currentResponse} /> : <TaskResponseForm task={current} />}
              </div>
            </Card>
          ) : (
            <Card>
              <EmptyState
                icon="target"
                title="Задания на этой неделе нет"
                actions={
                  blocker?.action && (
                    <ButtonLink to={blocker.action.to} variant="primary">
                      {blocker.action.label}
                    </ButtonLink>
                  )
                }
              >
                {blocker?.text} Новое задание появится {formatDate(period.to)}.
              </EmptyState>
            </Card>
          )}
          <Card title="Прошлые задания">
            {history.length === 0 ? (
              <p className="muted">Здесь появятся ваши ответы и оценки компаний.</p>
            ) : (
              <ul className="stack" style={{ margin: 0, padding: 0, listStyle: 'none' }}>
                {history.map((item) => (
                  <li key={item.id}>
                    <Card as="article" headingLevel={3}>
                      <div className="stack">
                        <div className="stack" style={{ gap: '0.25rem' }}>
                          <h3 style={{ margin: 0, fontSize: '1.0625rem' }}>{item.task.title}</h3>
                          <span className="small muted">
                            {item.task.company.displayName} · {skillLabel(meta, item.task.skillId)}
                          </span>
                        </div>
                        <ResponseView response={item} />
                      </div>
                    </Card>
                  </li>
                ))}
              </ul>
            )}
          </Card>
        </div>
        <div className="stack-lg">
          <Card title="Актуальность профиля">
            <div className="stack">
              <div>
                <FreshnessChip freshness={freshness} />
              </div>
              <p className="small">
                Профиль считается актуальным {freshness.windowDays} дней после подтверждения грейда или ответа на
                задание, который компания не вернула на доработку. Работодатели видят эту отметку в обезличенном
                профиле.
              </p>
            </div>
          </Card>
          <Card title="Как это работает" tinted>
            <ul className="small stack" style={{ margin: 0, paddingLeft: '1.25rem' }}>
              <li>Одно задание в неделю, по навыкам с подтверждениями и не выше вашего грейда.</li>
              <li>Можно решить задачу или описать подход — оба варианта засчитываются.</li>
              <li>Ответ видит только компания — автор задания; другие видят лишь отметку об актуальности.</li>
              <li>Задания не меняют грейд и подтверждения навыков: для этого есть тест знаний.</li>
            </ul>
          </Card>
        </div>
      </div>
    </>
  );
}
