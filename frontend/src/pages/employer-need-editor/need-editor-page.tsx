import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import { candidatesKey } from '@/entities/candidate';
import { companyQuery } from '@/entities/company';
import { type SkillId, skillLabel, useMeta } from '@/entities/meta';
import { createNeed, defaultEvidencePolicy, describePolicy, type Need, needsKey, updateNeed } from '@/entities/need';
import { useScope } from '@/entities/session';
import { describeError, isApiError } from '@/shared/api';
import {
  criticalityLabel,
  formatRub,
  gradeLabel,
  gradeOrder,
  skillOptions,
  specializationLabel,
  specializationOptions,
  taxBasisLabel,
  useDocumentTitle,
  workModeLabel,
  workModeOrder,
} from '@/shared/lib';
import {
  Button,
  ButtonLink,
  Callout,
  Card,
  Icon,
  PageHeader,
  SelectField,
  TextareaField,
  TextField,
  UnsavedChangesGuard,
} from '@/shared/ui';
import styles from './need-editor-page.module.css';
import {
  emptyRequirement,
  formFromNeed,
  type NeedFormValues,
  parseRubles,
  type RequirementForm,
  requirementsError,
  rublesError,
  salaryRangeError,
  toNeedInput,
} from './need-form';
import { PolicyEditor } from './policy-editor';

const requirementCodes = new Set([
  'duplicate_skill',
  'required_skill_missing',
  'empty_evidence_policy',
  'competition_required',
  'unknown_competition',
  'ambiguous_competition_policy',
]);

type RowProps = {
  index: number;
  value: RequirementForm;
  taken: Array<SkillId | ''>;
  onChange: (value: RequirementForm) => void;
  onRemove: () => void;
  canRemove: boolean;
};

function RequirementRow({ index, value, taken, onChange, onRemove, canRemove }: RowProps) {
  const meta = useMeta();
  const [open, setOpen] = useState(false);
  const label = value.skillId ? skillLabel(meta, value.skillId) : `Требование ${index + 1}`;
  return (
    <li className={styles.requirement}>
      <div className={styles.requirementRow}>
        <SelectField<SkillId>
          id={`${value.key}-skill`}
          label="Навык"
          placeholder="Выберите навык"
          value={value.skillId}
          onValueChange={(skillId) => onChange({ ...value, skillId })}
          options={skillOptions.map((skill) => ({
            ...skill,
            disabled: skill.value !== value.skillId && taken.includes(skill.value),
          }))}
        />
        <SelectField
          id={`${value.key}-criticality`}
          label="Приоритет"
          value={value.criticality}
          onValueChange={(criticality) => onChange({ ...value, criticality })}
          options={(['required', 'desired'] as const).map((item) => ({ value: item, label: criticalityLabel[item] }))}
        />
        <SelectField
          id={`${value.key}-grade`}
          label="Уровень не ниже"
          value={value.minimumGrade}
          onValueChange={(minimumGrade) => onChange({ ...value, minimumGrade })}
          options={gradeOrder.map((grade) => ({ value: grade, label: gradeLabel[grade] }))}
        />
      </div>
      <div className={styles.rowFooter}>
        <p className="small muted">Принимаются: {describePolicy(value.evidencePolicy)}</p>
        <div className={styles.rowActions}>
          <Button
            variant="quiet"
            icon={open ? 'chevronUp' : 'sliders'}
            aria-expanded={open}
            aria-controls={`${value.key}-policy`}
            onClick={() => setOpen((current) => !current)}
          >
            Подтверждения
          </Button>
          <Button
            variant="quiet"
            icon="trash"
            aria-label={`Удалить: ${label}`}
            disabled={!canRemove}
            onClick={onRemove}
          />
        </div>
      </div>
      {open && (
        <div id={`${value.key}-policy`} className={styles.policy}>
          <PolicyEditor
            policy={value.evidencePolicy}
            skillId={value.skillId}
            onChange={(evidencePolicy) => onChange({ ...value, evidencePolicy })}
          />
        </div>
      )}
    </li>
  );
}

function Summary({ values, companyName }: { values: NeedFormValues; companyName: string }) {
  const meta = useMeta();
  const labelOf = (item: RequirementForm) => (item.skillId ? skillLabel(meta, item.skillId) : 'не выбран');
  const required = values.requirements.filter((item) => item.criticality === 'required');
  const desired = values.requirements.filter((item) => item.criticality === 'desired');
  const from = parseRubles(values.salaryFrom);
  const to = parseRubles(values.salaryTo);
  return (
    <ul className={styles.summary}>
      <li>
        <Icon name="building" />
        <span>
          <strong>{companyName || 'Компания'}</strong>
          <br />
          <span className="muted">{values.title || 'Название не указано'}</span>
        </span>
      </li>
      <li>
        <Icon name="pin" />
        <span>
          {workModeLabel[values.workMode]} · {specializationLabel[values.specialization]} · {gradeLabel[values.grade]}
        </span>
      </li>
      <li>
        <Icon name="ruble" />
        <span>
          <strong>
            {'value' in from && 'value' in to
              ? `${formatRub(from.value)} – ${formatRub(to.value)} ₽/мес`
              : 'Вилка не указана'}
          </strong>
          <br />
          <span className="small muted">{taxBasisLabel.gross}</span>
        </span>
      </li>
      <li>
        <Icon name="list" />
        <span>
          Обязательных требований: {required.length}
          <br />
          <span className="small muted">{required.map(labelOf).join(', ') || '—'}</span>
        </span>
      </li>
      <li>
        <Icon name="layers" />
        <span>
          Желательных: {desired.length}
          <br />
          <span className="small muted">{desired.map(labelOf).join(', ') || '—'}</span>
        </span>
      </li>
    </ul>
  );
}

export function EmployerNeedEditorPage({ need }: { need?: Need }) {
  useDocumentTitle(need ? 'Редактирование потребности' : 'Новая потребность');
  const scope = useScope();
  const meta = useMeta();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const company = useQuery(companyQuery(scope));
  const [defaultPolicy, setDefaultPolicy] = useState(defaultEvidencePolicy);
  const [afterSave, setAfterSave] = useState<'search' | 'stay'>('search');

  const save = useMutation({
    mutationFn: (values: NeedFormValues) =>
      need ? updateNeed(need.id, toNeedInput(values)) : createNeed(toNeedInput(values)),
    onSuccess: async (saved, values) => {
      form.reset(values);
      queryClient.setQueryData([...needsKey(scope), saved.id], saved);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: needsKey(scope) }),
        queryClient.invalidateQueries({ queryKey: candidatesKey(scope) }),
      ]);
      if (afterSave === 'search') {
        await navigate({ to: '/employer/candidates', search: { needId: saved.id, mode: 'recommended' } });
      } else if (!need) {
        await navigate({ to: '/employer/needs/$needId', params: { needId: saved.id } });
      }
    },
  });

  const form = useForm({
    defaultValues: formFromNeed(need),
    onSubmit: ({ value }) => save.mutateAsync(value).catch(() => {}),
    onSubmitInvalid: () =>
      requestAnimationFrame(() =>
        document.querySelector<HTMLElement>('[aria-invalid="true"], [data-invalid="true"]')?.focus(),
      ),
  });

  const serverError = save.error;
  const salaryServerError =
    isApiError(serverError) && serverError.code === 'salary_range_invalid' ? serverError.message : undefined;
  const requirementServerError =
    isApiError(serverError) && requirementCodes.has(serverError.code) ? serverError.message : undefined;
  const genericError = serverError && !salaryServerError && !requirementServerError ? serverError : null;

  const submit = (mode: 'search' | 'stay') => {
    setAfterSave(mode);
    void form.handleSubmit();
  };

  return (
    <>
      <PageHeader
        title={need ? need.title : 'Новая потребность'}
        subtitle="Опишите роль, требования и условия. По потребности строится подборка и отправляются приглашения."
      />
      <form
        className="split"
        noValidate
        onSubmit={(event) => {
          event.preventDefault();
          submit('search');
        }}
      >
        <form.Subscribe selector={(state) => state.isDirty}>
          {(dirty) => <UnsavedChangesGuard when={dirty && !save.isPending} />}
        </form.Subscribe>
        <Card>
          <div className="stack-lg">
            <form.Field
              name="title"
              validators={{ onSubmit: ({ value }) => (value.trim() ? undefined : 'Укажите название') }}
            >
              {(field) => (
                <TextField
                  id="need-title"
                  label="Название"
                  required
                  maxLength={160}
                  value={field.state.value}
                  onChange={(event) => field.handleChange(event.target.value)}
                  error={field.state.meta.errors[0] as string | undefined}
                />
              )}
            </form.Field>
            <div className={styles.triple}>
              <form.Field name="specialization">
                {(field) => (
                  <SelectField
                    id="need-specialization"
                    label="Специализация"
                    required
                    value={field.state.value}
                    onValueChange={field.handleChange}
                    options={specializationOptions}
                  />
                )}
              </form.Field>
              <form.Field name="grade">
                {(field) => (
                  <SelectField
                    id="need-grade"
                    label="Грейд"
                    required
                    value={field.state.value}
                    onValueChange={field.handleChange}
                    options={gradeOrder.map((value) => ({ value, label: gradeLabel[value] }))}
                  />
                )}
              </form.Field>
              <form.Field name="workMode">
                {(field) => (
                  <SelectField
                    id="need-mode"
                    label="Формат"
                    required
                    value={field.state.value}
                    onValueChange={field.handleChange}
                    options={workModeOrder.map((value) => ({ value, label: workModeLabel[value] }))}
                  />
                )}
              </form.Field>
            </div>
            <fieldset className={styles.salary}>
              <legend>
                Зарплата, ₽/мес{' '}
                <span aria-hidden="true" style={{ color: 'var(--fsp-color-danger)' }}>
                  *
                </span>
              </legend>
              <form.Field name="salaryFrom" validators={{ onSubmit: ({ value }) => rublesError(value) }}>
                {(field) => (
                  <TextField
                    id="need-salary-from"
                    label="От"
                    inputMode="numeric"
                    autoComplete="off"
                    value={field.state.value}
                    onChange={(event) => {
                      field.handleChange(event.target.value);
                      save.reset();
                    }}
                    error={field.state.meta.errors[0] as string | undefined}
                  />
                )}
              </form.Field>
              <form.Field
                name="salaryTo"
                validators={{
                  onSubmit: ({ value, fieldApi }) =>
                    rublesError(value) ?? salaryRangeError(fieldApi.form.getFieldValue('salaryFrom'), value),
                }}
              >
                {(field) => (
                  <TextField
                    id="need-salary-to"
                    label="До"
                    inputMode="numeric"
                    autoComplete="off"
                    value={field.state.value}
                    onChange={(event) => {
                      field.handleChange(event.target.value);
                      save.reset();
                    }}
                    error={(field.state.meta.errors[0] as string | undefined) ?? salaryServerError}
                  />
                )}
              </form.Field>
              <div className={styles.basis}>
                <span className="small muted">Основа расчёта</span>
                <strong>{taxBasisLabel.gross}</strong>
                <span className="small muted">Рубли в месяц, целые суммы</span>
              </div>
            </fieldset>
            <form.Field
              name="description"
              validators={{ onSubmit: ({ value }) => (value.trim() ? undefined : 'Кратко опишите задачи команды') }}
            >
              {(field) => (
                <TextareaField
                  id="need-description"
                  label="Краткое описание"
                  required
                  maxLength={6000}
                  value={field.state.value}
                  onChange={(event) => field.handleChange(event.target.value)}
                  error={field.state.meta.errors[0] as string | undefined}
                />
              )}
            </form.Field>

            <section className="stack" aria-labelledby="need-requirements">
              <div>
                <h2 id="need-requirements" style={{ margin: 0, fontSize: '1.25rem' }}>
                  Требования
                </h2>
                <p className="small muted">
                  Обязательные требования определяют соответствие; желательные не компенсируют отсутствие обязательных.
                  Требование выполнено, когда навык подтверждён на нужном уровне принимаемым источником.
                </p>
              </div>
              <details className={styles.defaults}>
                <summary>Подтверждения по умолчанию для новых требований</summary>
                <div className="stack" style={{ paddingTop: '0.75rem' }}>
                  <PolicyEditor policy={defaultPolicy} onChange={setDefaultPolicy} />
                  <form.Field name="requirements" mode="array">
                    {(field) => (
                      <Button
                        size="sm"
                        onClick={() =>
                          field.setValue(
                            field.state.value.map((item) => ({
                              ...item,
                              evidencePolicy: structuredClone(defaultPolicy),
                            })),
                          )
                        }
                      >
                        Применить ко всем требованиям
                      </Button>
                    )}
                  </form.Field>
                </div>
              </details>
              <form.Field
                name="requirements"
                mode="array"
                validators={{ onSubmit: ({ value }) => requirementsError(value) }}
              >
                {(field) => {
                  const error = (field.state.meta.errors[0] as string | undefined) ?? requirementServerError;
                  return (
                    <div className="stack">
                      <ul
                        className={styles.requirements}
                        data-invalid={error ? true : undefined}
                        tabIndex={error ? -1 : undefined}
                      >
                        {field.state.value.map((requirement, index) => (
                          <RequirementRow
                            key={requirement.key}
                            index={index}
                            value={requirement}
                            taken={field.state.value.map((item) => item.skillId)}
                            canRemove={field.state.value.length > 1}
                            onChange={(next) => {
                              field.replaceValue(index, next);
                              save.reset();
                            }}
                            onRemove={() => field.removeValue(index)}
                          />
                        ))}
                      </ul>
                      {error && (
                        <Callout tone="danger" role="alert">
                          {error}
                        </Callout>
                      )}
                      <Button
                        variant="quiet"
                        icon="plus"
                        disabled={field.state.value.length >= Math.min(5, meta.skills.length)}
                        onClick={() => field.pushValue(emptyRequirement(defaultPolicy, form.getFieldValue('grade')))}
                      >
                        Добавить требование
                      </Button>
                    </div>
                  );
                }}
              </form.Field>
            </section>
          </div>
        </Card>

        <div className="stack-lg">
          <Card>
            <form.Subscribe selector={(state) => state.values}>
              {(values) => <Summary values={values} companyName={company.data?.displayName ?? ''} />}
            </form.Subscribe>
            <hr className="divider" />
            <p className="small muted" style={{ margin: '0.75rem 0' }}>
              {!meta.capabilities.vacancyPublication
                ? 'Публиковать вакансию не нужно: кандидатов из подборки можно пригласить напрямую.'
                : need?.published
                  ? 'Вакансия опубликована: кандидаты увидят изменения сразу, полученные отклики сохранят условия на момент отклика.'
                  : 'Приглашать кандидатов можно без публикации. Опубликовать потребность как вакансию можно в списке потребностей.'}
            </p>
            {company.data && !company.data.readyToInvite && (
              <Callout
                tone="warning"
                title="Профиль компании заполнен не полностью"
                actions={
                  <ButtonLink to="/employer/company" size="sm">
                    Заполнить профиль
                  </ButtonLink>
                }
              >
                Искать кандидатов можно уже сейчас, а приглашения станут доступны после заполнения профиля.
              </Callout>
            )}
            {genericError && (
              <Callout tone="danger" role="alert" title="Потребность не сохранена">
                {describeError(genericError).message} Введённые данные остались в форме.
              </Callout>
            )}
            <div className="stack" style={{ marginTop: '1rem' }}>
              <Button
                type="submit"
                variant="primary"
                block
                iconAfter="arrowRight"
                pending={save.isPending && afterSave === 'search'}
                disabled={save.isPending}
              >
                Сохранить и найти кандидатов
              </Button>
              <Button
                block
                pending={save.isPending && afterSave === 'stay'}
                disabled={save.isPending}
                onClick={() => submit('stay')}
              >
                Сохранить
              </Button>
            </div>
          </Card>
          {need && (
            <p className="small muted">
              Изменения не затрагивают уже отправленные приглашения: их условия зафиксированы на момент отправки.
            </p>
          )}
        </div>
      </form>
    </>
  );
}
