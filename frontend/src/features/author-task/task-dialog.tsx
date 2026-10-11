import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import type { SkillId } from '@/entities/meta';
import { useScope } from '@/entities/session';
import { createTask, type TaskInput, tasksKey } from '@/entities/task';
import { describeError, isApiError } from '@/shared/api';
import { gradeLabel, gradeOrder, skillOptions } from '@/shared/lib';
import { Button, ButtonLink, Callout, Dialog, SelectField, TextareaField, TextField, useToast } from '@/shared/ui';

type Props = { open: boolean; onClose: () => void };

const empty = {
  title: '',
  prompt: '',
  skillId: '' as SkillId | '',
  grade: 'junior' as TaskInput['grade'],
  estimate: '30',
};

/** A short task the platform offers weekly to candidates with evidence for its skill at or above its grade. */
export function TaskDialog({ open, onClose }: Props) {
  const scope = useScope();
  const queryClient = useQueryClient();
  const toast = useToast();
  const [values, setValues] = useState(empty);
  const [touched, setTouched] = useState(false);
  const minutes = Number(values.estimate);
  const errors = {
    title: values.title.trim() ? undefined : 'Укажите название',
    prompt: values.prompt.trim() ? undefined : 'Опишите задачу',
    skillId: values.skillId ? undefined : 'Выберите навык',
    estimate:
      Number.isInteger(minutes) && minutes >= 5 && minutes <= 120 ? undefined : 'От 5 до 120 минут, целое число',
  };
  const valid = Object.values(errors).every((error) => error === undefined);
  const create = useMutation({
    mutationFn: () =>
      createTask({
        title: values.title.trim(),
        prompt: values.prompt.trim(),
        skillId: values.skillId as SkillId,
        grade: values.grade,
        estimateMinutes: minutes,
      }),
    onSuccess: async (task) => {
      await queryClient.invalidateQueries({ queryKey: tasksKey(scope) });
      toast(`Задание «${task.title}» добавлено`);
      setValues(empty);
      setTouched(false);
      onClose();
    },
  });
  const submit = () => {
    setTouched(true);
    if (valid) create.mutate();
  };
  const shown = (field: keyof typeof errors) => (touched ? errors[field] : undefined);
  return (
    <Dialog
      open={open}
      onClose={onClose}
      wide
      title="Новое задание"
      description="Платформа раз в неделю предлагает его кандидатам с подтверждённым грейдом не ниже указанного и свидетельствами по навыку."
      footer={
        <>
          <Button onClick={onClose} disabled={create.isPending}>
            Отмена
          </Button>
          <Button variant="primary" pending={create.isPending} pendingLabel="Сохраняем…" onClick={submit}>
            Добавить задание
          </Button>
        </>
      }
    >
      <form
        className="stack"
        noValidate
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        <TextField
          id="task-title"
          label="Название"
          required
          maxLength={160}
          value={values.title}
          error={shown('title')}
          onChange={(event) => setValues({ ...values, title: event.target.value })}
        />
        <TextareaField
          id="task-prompt"
          label="Условие"
          required
          rows={6}
          maxLength={4000}
          value={values.prompt}
          error={shown('prompt')}
          hint="Короткая задача на 5–120 минут: кандидат решает её или описывает подход"
          onChange={(event) => setValues({ ...values, prompt: event.target.value })}
        />
        <div className="form-grid">
          <SelectField
            id="task-skill"
            label="Навык"
            required
            placeholder="Выберите навык"
            value={values.skillId}
            error={shown('skillId')}
            onValueChange={(skillId) => setValues({ ...values, skillId })}
            options={skillOptions}
          />
          <SelectField
            id="task-grade"
            label="Минимальный грейд"
            value={values.grade}
            onValueChange={(grade) => setValues({ ...values, grade })}
            options={gradeOrder.map((value) => ({ value, label: gradeLabel[value] }))}
          />
          <TextField
            id="task-estimate"
            label="Время, минут"
            inputMode="numeric"
            value={values.estimate}
            error={shown('estimate')}
            onChange={(event) => setValues({ ...values, estimate: event.target.value })}
          />
        </div>
        {create.error && (
          <Callout
            tone="danger"
            role="alert"
            actions={
              isApiError(create.error, 422) && (
                <ButtonLink to="/employer/company" size="sm">
                  Заполнить профиль компании
                </ButtonLink>
              )
            }
          >
            Задание не сохранено: {describeError(create.error).message}
          </Callout>
        )}
      </form>
    </Dialog>
  );
}
