import { useForm } from '@tanstack/react-form';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useRef, useState } from 'react';
import { type Job, useJob } from '@/entities/job';
import type { SkillId } from '@/entities/meta';
import { profileKey } from '@/entities/profile';
import { useScope } from '@/entities/session';
import { type components, describeError, isApiError } from '@/shared/api';
import { gradeLabel, gradeOrder, skillOptions, specializationOptions } from '@/shared/lib';
import {
  Button,
  Callout,
  Dialog,
  InlineStatus,
  SelectField,
  TextareaField,
  TextField,
  TokenInput,
  TokenPicker,
  useToast,
} from '@/shared/ui';
import { confirmProfileImport, importProfilePdf, validatePdf } from './api';

type Preview = components['schemas']['ImportPreview'];
const fieldLabels: Record<string, string> = {
  name: 'Имя',
  specialization: 'Специализация',
  selectedGrade: 'Грейд для проверки',
  stack: 'Стек',
  teamRole: 'Роль в команде',
  experienceMonths: 'Стаж',
  experienceSince: 'В IT с года',
  experience: 'Опыт',
  softSkills: 'Навыки общения',
};

type ImportResult = Extract<NonNullable<Job['result']>, { kind: 'profile_import' }>;

function PreviewForm({ jobId, result, onDone }: { jobId: string; result: ImportResult; onDone: () => void }) {
  const scope = useScope();
  const queryClient = useQueryClient();
  const toast = useToast();
  const confirm = useMutation({
    mutationFn: (profile: Preview) => confirmProfileImport(jobId, profile),
    onSuccess: (profile) => {
      queryClient.setQueryData(profileKey(scope), profile);
      toast('Данные из PDF применены к профилю');
      onDone();
    },
    onError: () => {
      void queryClient.invalidateQueries({ queryKey: profileKey(scope) });
    },
  });
  const form = useForm({
    defaultValues: result.preview,
    onSubmit: ({ value }) => confirm.mutateAsync({ ...value, name: value.name.trim() }).catch(() => {}),
  });
  return (
    <form
      className="stack"
      onSubmit={(event) => {
        event.preventDefault();
        void form.handleSubmit();
      }}
    >
      {result.simulation && (
        <Callout tone="warning" title="Демонстрационное извлечение">
          В демонстрационном режиме содержимое файла не анализируется: используется подготовленный пример. Проверьте и
          исправьте поля перед применением. Неизвестные значения оставлены пустыми.
        </Callout>
      )}
      {!result.simulation && (
        <Callout tone="info" title="Проверьте распознанные данные">
          Поля заполнены по тексту PDF с помощью ИИ. Возможны ошибки: сверьте значения перед применением. Если поле не
          найдено, сохранено его текущее значение. Подтверждённый грейд не изменится.
        </Callout>
      )}
      {result.evidence && result.evidence.length > 0 && (
        <details>
          <summary>Фрагменты PDF, использованные при заполнении</summary>
          <ul className="stack">
            {result.evidence.map((item) => (
              <li key={`${item.field}-${item.page}-${item.quote}`}>
                <strong>
                  {fieldLabels[item.field]} · страница {item.page}
                </strong>
                <p style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{item.quote}</p>
              </li>
            ))}
          </ul>
        </details>
      )}
      <div className="form-grid">
        <form.Field name="name" validators={{ onSubmit: ({ value }) => (value.trim() ? undefined : 'Укажите имя') }}>
          {(field) => (
            <TextField
              id="import-name"
              label="Имя и фамилия"
              value={field.state.value}
              maxLength={120}
              onChange={(event) => field.handleChange(event.target.value)}
              error={field.state.meta.errors[0] as string | undefined}
            />
          )}
        </form.Field>
        <form.Field name="teamRole">
          {(field) => (
            <TextField
              id="import-role"
              label="Роль в команде"
              value={field.state.value}
              maxLength={120}
              onChange={(event) => field.handleChange(event.target.value)}
            />
          )}
        </form.Field>
        <form.Field name="specialization">
          {(field) => (
            <SelectField
              id="import-specialization"
              label="Специализация"
              value={field.state.value}
              onValueChange={field.handleChange}
              options={specializationOptions}
            />
          )}
        </form.Field>
        <form.Field name="selectedGrade">
          {(field) => (
            <SelectField
              id="import-grade"
              label="Грейд для проверки"
              hint="Грейд из резюме — только цель. Подтверждённый грейд не меняется."
              value={field.state.value}
              onValueChange={field.handleChange}
              options={gradeOrder.map((value) => ({ value, label: gradeLabel[value] }))}
            />
          )}
        </form.Field>
        <form.Field name="experienceSince">
          {(field) => (
            <TextField
              id="import-since"
              label="В IT с какого года"
              inputMode="numeric"
              maxLength={4}
              hint="Из резюме: учёба, первая работа или «программирую с …». Пусто — год не указан"
              value={field.state.value === null ? '' : String(field.state.value)}
              onChange={(event) => {
                const digits = event.target.value.replace(/\D/g, '').slice(0, 4);
                field.handleChange(digits === '' ? null : Number(digits));
              }}
            />
          )}
        </form.Field>
        <form.Field name="experienceMonths">
          {(field) => (
            <TextField
              id="import-months"
              label="Стаж, месяцев"
              inputMode="numeric"
              hint="Если в резюме указан общий стаж в месяцах. Год начала в IT важнее"
              value={field.state.value === null ? '' : String(field.state.value)}
              onChange={(event) => {
                const digits = event.target.value.replace(/\D/g, '');
                field.handleChange(digits === '' ? null : Math.min(1200, Number(digits)));
              }}
            />
          )}
        </form.Field>
        <div className="full">
          <form.Field name="stack">
            {(field) => (
              <TokenPicker<SkillId>
                id="import-stack"
                label="Стек"
                max={12}
                placeholder="Найти технологию"
                options={skillOptions}
                selected={field.state.value}
                onChange={field.handleChange}
              />
            )}
          </form.Field>
        </div>
        <div className="full">
          <form.Field name="experience">
            {(field) => (
              <TextareaField
                id="import-experience"
                label="Опыт"
                maxLength={4000}
                value={field.state.value}
                onChange={(event) => field.handleChange(event.target.value)}
              />
            )}
          </form.Field>
        </div>
        <div className="full">
          <form.Field name="softSkills">
            {(field) => (
              <TokenInput
                id="import-soft"
                label="Навыки общения"
                aside="Самооценка"
                max={12}
                maxLength={100}
                values={field.state.value}
                onChange={field.handleChange}
              />
            )}
          </form.Field>
        </div>
      </div>
      {confirm.error && <Callout tone="danger">{describeError(confirm.error).message}</Callout>}
      <div className="row" style={{ justifyContent: 'flex-end' }}>
        <Button onClick={onDone}>Отменить импорт</Button>
        <Button type="submit" variant="primary" pending={confirm.isPending} pendingLabel="Применяем…">
          Применить к профилю
        </Button>
      </div>
    </form>
  );
}

export function ImportProfileDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const scope = useScope();
  const [fileError, setFileError] = useState<string | null>(null);
  const [jobId, setJobId] = useState<string | null>(null);
  const generation = useRef(0);
  const upload = useMutation({
    mutationFn: importProfilePdf,
    onMutate: () => generation.current,
    onSuccess: (job, _file, started) => {
      if (started === generation.current) setJobId(job.id);
    },
  });
  const job = useJob(scope, jobId);

  const close = () => {
    generation.current += 1;
    setJobId(null);
    setFileError(null);
    upload.reset();
    onClose();
  };

  const result =
    job.data?.status === 'succeeded' && job.data.result?.kind === 'profile_import' ? job.data.result : null;

  return (
    <Dialog
      open={open}
      onClose={close}
      wide
      title="Импорт профиля из PDF"
      description="Поля из файла попадут в форму для проверки. Профиль изменится только после подтверждения."
    >
      {!jobId && (
        <div className="stack">
          <label className="stack" style={{ gap: '0.5rem' }}>
            <strong>Файл резюме</strong>
            <input
              type="file"
              accept="application/pdf,.pdf"
              aria-describedby="import-file-hint"
              disabled={upload.isPending}
              onChange={(event) => {
                const file = event.target.files?.[0];
                event.target.value = '';
                if (!file) return;
                const problem = validatePdf(file);
                setFileError(problem);
                if (!problem) upload.mutate(file);
              }}
            />
            <span id="import-file-hint" className="small muted">
              PDF с текстовым слоем до 2 МиБ и 20 страниц. Текст анализируется для заполнения формы. Сканы и файлы с
              паролем не поддерживаются.
            </span>
          </label>
          {upload.isPending && <InlineStatus>Загружаем и разбираем PDF…</InlineStatus>}
          {(fileError || upload.error) && (
            <Callout tone="danger" role="alert" title="Файл не принят">
              {fileError ??
                (isApiError(upload.error) && upload.error.code === 'resume_extraction_invalid'
                  ? upload.error.message
                  : describeError(upload.error).message)}
            </Callout>
          )}
        </div>
      )}
      {jobId && !job.data && job.isPending && <InlineStatus>Файл загружен, ставим в очередь разбора…</InlineStatus>}
      {jobId && job.data && !result && job.data.status !== 'failed' && (
        <InlineStatus>{job.data.status === 'queued' ? 'Файл в очереди на разбор…' : 'Разбираем файл…'}</InlineStatus>
      )}
      {jobId && (job.data?.status === 'failed' || job.isError) && (
        <Callout
          tone="danger"
          role="alert"
          title="Не удалось разобрать PDF"
          actions={
            <>
              <Button size="sm" onClick={() => setJobId(null)}>
                Выбрать другой файл
              </Button>
              <Button size="sm" variant="quiet" onClick={close}>
                Заполнить вручную
              </Button>
            </>
          }
        >
          {job.data?.error?.message ?? describeError(job.error).message} Данные профиля не изменены.
        </Callout>
      )}
      {jobId && result && <PreviewForm jobId={jobId} result={result} onDone={close} />}
    </Dialog>
  );
}
