import { useMutation, useQueryClient, useSuspenseQuery } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useCallback, useEffect, useRef, useState } from 'react';
import {
  type Assessment,
  type AssessmentAnswer,
  askAssistant,
  assessmentKey,
  assessmentQuery,
  assessmentsKey,
  isEditable,
  isEvaluating,
  saveAnswer,
  submitAssessment,
} from '@/entities/assessment';
import { useMeta } from '@/entities/meta';
import { profileKey } from '@/entities/profile';
import { useScope } from '@/entities/session';
import { graphKey, skillsKey } from '@/entities/skill';
import { vacanciesKey } from '@/entities/vacancy';
import { describeError, isApiError, newIdempotencyKey } from '@/shared/api';
import { formatTime, gradeLabel, specializationLabel, useDocumentTitle } from '@/shared/lib';
import {
  Button,
  ButtonLink,
  Callout,
  Card,
  Checkbox,
  ConfirmDialog,
  Icon,
  panelId,
  StatusChip,
  type TabItem,
  Tabs,
  TextareaField,
  UnsavedChangesGuard,
} from '@/shared/ui';
import styles from './workspace-page.module.css';

type SaveState = { status: 'saved' | 'dirty' | 'saving' | 'error'; at: Date | null; error?: unknown };
type ChatMessage = { id: number; from: 'user' | 'assistant'; text: string; simulation: boolean };

const AUTOSAVE_DELAY = 1200;
function TaskPanel({ assessment }: { assessment: Assessment }) {
  const [tab, setTab] = useState<'task' | 'materials'>('task');
  const task = assessment.task;
  const tabs: TabItem<'task' | 'materials'>[] = [
    { id: 'task', label: 'Условие' },
    { id: 'materials', label: 'Рубрика' },
  ];
  return (
    <Card className={styles.pane}>
      <Tabs items={tabs} value={tab} onChange={setTab} label="Материалы задачи" idPrefix="task" />
      <div
        role="tabpanel"
        id={panelId('task', tab)}
        aria-labelledby={`task-tab-${tab}`}
        className="stack"
        style={{ paddingTop: '1rem' }}
      >
        {tab === 'task' ? (
          <>
            <h2 style={{ margin: 0, fontSize: '1.25rem' }}>{task.title}</h2>
            <p>{task.prompt}</p>
            <h3 style={{ margin: 0, fontSize: '1rem' }}>Требования</h3>
            <ol className={styles.criteria}>
              {task.acceptanceCriteria.map((criterion) => (
                <li key={criterion}>
                  <span>{criterion}</span>
                </li>
              ))}
            </ol>
          </>
        ) : (
          <>
            <p className="small muted">
              Семейство задач «{task.family}», версия {task.version}, вариант {task.variantId}. Рубрика общая для всех
              вариантов семейства.
            </p>
            <ul className="stack" style={{ margin: 0, paddingLeft: '1.25rem' }}>
              {task.rubric.map((criterion) => (
                <li key={criterion.id}>
                  {criterion.label} <span className="small muted">· вес {criterion.weight}</span>
                </li>
              ))}
            </ul>
          </>
        )}
      </div>
    </Card>
  );
}

function AssistantPanel({
  assessment,
  disabled,
  prepare,
}: {
  assessment: Assessment;
  disabled: boolean;
  prepare: () => Promise<boolean>;
}) {
  const meta = useMeta();
  const available = meta.mode === 'mock' || meta.capabilities.realLlm;
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [draft, setDraft] = useState('');
  const nextId = useRef(1);
  const ask = useMutation({
    mutationFn: async (message: string) => {
      if (!(await prepare())) throw new Error('Сначала сохраните ответы');
      return askAssistant(assessment.id, message);
    },
    onSuccess: (reply, question) => {
      setDraft('');
      setMessages((items) => [
        ...items,
        { id: nextId.current++, from: 'user', text: question, simulation: false },
        { id: nextId.current++, from: 'assistant', text: reply.message, simulation: reply.simulation },
      ]);
    },
  });
  return (
    <Card className={styles.pane} title="ИИ-помощник" headingLevel={2}>
      <div className="stack">
        <p className="small muted">
          Помощник разрешён на всех этапах, как и внешние инструменты.
          {meta.mode === 'mock'
            ? ' В учебном API ответы подготовлены заранее.'
            : ' Ответы генерируются ИИ по вопросам теста, сохранённым ответам и текущему вопросу. Помощник не выставляет оценку.'}
        </p>
        {!available && <Callout tone="neutral">ИИ-помощник сейчас не подключён.</Callout>}
        {messages.length > 0 && (
          <ol className={styles.chat} aria-live="polite" aria-label="Переписка с помощником">
            {messages.map((message) => (
              <li
                key={message.id}
                className={`${styles.message} ${message.from === 'user' ? styles.fromUser : styles.fromAssistant}`}
              >
                <span className="visually-hidden">{message.from === 'user' ? 'Вы: ' : 'Помощник: '}</span>
                {message.text}
                {message.from === 'assistant' && message.simulation && (
                  <div className="small muted" style={{ marginTop: '0.5rem' }}>
                    Демонстрационный ответ
                  </div>
                )}
              </li>
            ))}
          </ol>
        )}
        {ask.error && (
          <Callout tone="warning">
            Помощник недоступен: {describeError(ask.error).message}. Выбранные ответы сохранены в черновике.
          </Callout>
        )}
        <form
          className="stack"
          onSubmit={(event) => {
            event.preventDefault();
            const text = draft.trim();
            if (!text || disabled || ask.isPending || !available) return;
            ask.mutate(text);
          }}
        >
          <TextareaField
            id="assistant-question"
            label="Вопрос по тесту"
            rows={3}
            maxLength={10000}
            value={draft}
            disabled={disabled || ask.isPending || !available}
            onChange={(event) => setDraft(event.target.value)}
          />
          <Button
            type="submit"
            icon="send"
            disabled={disabled || ask.isPending || !available || !draft.trim()}
            pending={ask.isPending}
            pendingLabel="Ждём ответ…"
          >
            Спросить
          </Button>
        </form>
      </div>
    </Card>
  );
}

function Workspace({ assessment }: { assessment: Assessment }) {
  const scope = useScope();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [answer, setAnswer] = useState<AssessmentAnswer>(assessment.answer);
  const [save, setSave] = useState<SaveState>({ status: 'saved', at: null });
  const [confirmSubmit, setConfirmSubmit] = useState(false);
  const [answerError, setAnswerError] = useState<string | null>(null);
  const lastSaved = useRef(JSON.stringify(assessment.answer));
  const submitKey = useRef<string | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const saveQueue = useRef<Promise<boolean>>(Promise.resolve(true));

  const latest = useRef(answer);
  latest.current = answer;
  const { mutateAsync: persistAnswer } = useMutation({
    mutationFn: (value: AssessmentAnswer) => saveAnswer(assessment.id, value),
  });

  const flush = useCallback(
    (value: AssessmentAnswer) => {
      // Compare with the acknowledged draft only after all earlier writes finish.
      const next = saveQueue.current.then(async () => {
        const serialized = JSON.stringify(value);
        if (serialized === lastSaved.current) return true;
        setSave((current) => ({ ...current, status: 'saving' }));
        try {
          const updated = await persistAnswer(value);
          lastSaved.current = serialized;
          queryClient.setQueryData(assessmentKey(scope, assessment.id), updated);
          setSave({ status: serialized === JSON.stringify(latest.current) ? 'saved' : 'dirty', at: new Date() });
          return true;
        } catch (error) {
          setSave((current) => ({ status: 'error', at: current.at, error }));
          if (isApiError(error, 409))
            void queryClient.invalidateQueries({ queryKey: assessmentKey(scope, assessment.id) });
          return false;
        }
      });
      saveQueue.current = next;
      return next;
    },
    [assessment.id, persistAnswer, queryClient, scope],
  );

  useEffect(() => {
    if (JSON.stringify(answer) === lastSaved.current) return;
    setSave((current) =>
      current.status === 'saving' || current.status === 'dirty' ? current : { ...current, status: 'dirty' },
    );
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => void flush(latest.current), AUTOSAVE_DELAY);
    return () => window.clearTimeout(timer.current);
  }, [answer, flush]);

  const submit = useMutation({
    mutationFn: async () => {
      if (!(await flush(latest.current))) throw new Error('Черновик не сохранён — отправка отменена');
      submitKey.current ??= newIdempotencyKey();
      return submitAssessment(assessment.id, submitKey.current);
    },
    onSuccess: async () => {
      setConfirmSubmit(false);
      await Promise.all(
        [
          assessmentKey(scope, assessment.id),
          assessmentsKey(scope),
          profileKey(scope),
          vacanciesKey(scope),
          skillsKey(scope),
          graphKey(scope),
        ].map((queryKey) => queryClient.invalidateQueries({ queryKey })),
      );
      await navigate({ to: '/candidate/assessments/$assessmentId/result', params: { assessmentId: assessment.id } });
    },
    onError: (error) => {
      // A server answer closes this logical submission; only transport failures keep the key for a retry.
      if (isApiError(error)) submitKey.current = null;
    },
  });

  const questions = assessment.task.questions ?? [];
  const responses = answer.responses ?? [];
  const requestSubmit = () => {
    if (questions.some((q) => !responses.some((r) => r.questionId === q.id))) {
      setAnswerError('Выберите ответ на каждый вопрос перед отправкой.');
      return;
    }
    setAnswerError(null);
    setConfirmSubmit(true);
  };

  return (
    <>
      <UnsavedChangesGuard when={save.status !== 'saved' && !submit.isSuccess}>
        <p>Последние изменения ещё не сохранены на сервере. Если уйти сейчас, они будут потеряны.</p>
      </UnsavedChangesGuard>
      <div className={styles.head}>
        <div>
          <h1 className={styles.title}>
            {specializationLabel[assessment.specialization]} · Проверка {gradeLabel[assessment.grade]}
          </h1>
          <p className="muted">
            Тест знаний · выбрано {responses.length} из {questions.length} ответов · версия {assessment.task.version}
          </p>
        </div>
        <div className={styles.badges}>
          <StatusChip tone="success" icon="sparkles">
            ИИ разрешён, включая внешние инструменты
          </StatusChip>
          <StatusChip tone="info" icon="info">
            Неудачная попытка не снизит подтверждённый грейд
          </StatusChip>
        </div>
      </div>
      <Callout tone="neutral" title="Проверка знаний">
        Код писать не нужно. Выберите один ответ на каждый вопрос. Результат относится к темам этого теста и не
        подтверждает практический опыт разработки.
      </Callout>
      <div className={styles.panes} style={{ marginTop: '1rem' }}>
        <TaskPanel assessment={assessment} />
        <div className={`${styles.pane} stack`}>
          {questions.map((question, index) => (
            <Card key={question.id}>
              <fieldset className={styles.question} disabled={submit.isPending}>
                <legend>
                  {index + 1}. {question.prompt}
                </legend>
                {question.options.map((option) => (
                  <label key={option.id} className={styles.option}>
                    <input
                      type="radio"
                      name={`question-${question.id}`}
                      value={option.id}
                      checked={responses.some((r) => r.questionId === question.id && r.optionId === option.id)}
                      onChange={() => {
                        setAnswerError(null);
                        setAnswer((value) => ({
                          ...value,
                          responses: [
                            ...(value.responses ?? []).filter((r) => r.questionId !== question.id),
                            { questionId: question.id, optionId: option.id },
                          ],
                        }));
                      }}
                    />
                    <span>{option.label}</span>
                  </label>
                ))}
              </fieldset>
            </Card>
          ))}
          <Checkbox
            checked={answer.externalAiUsed}
            disabled={submit.isPending}
            onChange={(event) => setAnswer((value) => ({ ...value, externalAiUsed: event.target.checked }))}
            label="Я использовал внешние ИИ-инструменты"
            description="Это разрешено и не снижает оценку."
          />
          {answerError && (
            <Callout tone="warning" role="alert">
              {answerError}
            </Callout>
          )}
        </div>
        <AssistantPanel assessment={assessment} disabled={submit.isPending} prepare={() => flush(latest.current)} />
      </div>
      <div className={styles.footer}>
        <span className="row small" role="status">
          <Icon
            name={save.status === 'error' ? 'alert' : save.status === 'saved' ? 'checkCircle' : 'clock'}
            size={18}
          />
          {save.status === 'saved' &&
            (save.at ? `Черновик сохранён в ${formatTime(save.at)}` : 'Черновик соответствует сохранённой версии')}
          {save.status === 'dirty' && 'Есть несохранённые изменения'}
          {save.status === 'saving' && 'Сохраняем черновик…'}
          {save.status === 'error' && (
            <span className={styles.saveError}>
              Черновик не сохранён: {describeError(save.error).message}{' '}
              <Button size="sm" variant="quiet" onClick={() => void flush(latest.current)}>
                Повторить
              </Button>
            </span>
          )}
        </span>
        <div className={styles.footerActions}>
          <Button variant="primary" icon="send" onClick={requestSubmit} disabled={submit.isPending}>
            Отправить ответы
          </Button>
        </div>
      </div>
      <ConfirmDialog
        open={confirmSubmit}
        title="Отправить ответы на оценку?"
        confirmLabel="Отправить"
        pending={submit.isPending}
        onConfirm={() => submit.mutate()}
        onCancel={() => {
          setConfirmSubmit(false);
          submit.reset();
        }}
      >
        <div className="stack">
          <p>После отправки ответы изменить нельзя. Подтверждённый грейд сохранится при любом результате.</p>
          {submit.error && (
            <Callout tone="danger" role="alert">
              {describeError(submit.error).message}
            </Callout>
          )}
        </div>
      </ConfirmDialog>
    </>
  );
}

export function AssessmentWorkspacePage({ assessmentId }: { assessmentId: string }) {
  useDocumentTitle('Тест знаний');
  const scope = useScope();
  const { data: assessment } = useSuspenseQuery(assessmentQuery(scope, assessmentId));
  if (assessment.task.execution !== 'questionnaire') {
    return (
      <Card title="Архивная демонстрационная задача">
        <div className="stack">
          <p>
            Формат изменился: теперь проверка состоит из тестовых вопросов. Старое решение сохранено в истории и больше
            не оценивается.
          </p>
          <pre className={styles.legacyAnswer}>{assessment.answer.code}</pre>
          <p>{assessment.answer.explanation}</p>
          <ButtonLink to="/candidate/assessments">Перейти к тестам знаний</ButtonLink>
        </div>
      </Card>
    );
  }
  if (!isEditable(assessment)) {
    return (
      <Card title={isEvaluating(assessment) ? 'Решение отправлено' : 'Проверка завершена'}>
        <div className="stack">
          <p>Эта попытка уже отправлена, редактировать решение нельзя.</p>
          <ButtonLink
            to="/candidate/assessments/$assessmentId/result"
            params={{ assessmentId }}
            variant="primary"
            iconAfter="arrowRight"
          >
            Открыть результат
          </ButtonLink>
        </div>
      </Card>
    );
  }
  return <Workspace key={assessment.id} assessment={assessment} />;
}
