import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { type Profile, profileKey, updateProfile, yearsInIt } from '@/entities/profile';
import { useScope } from '@/entities/session';
import { vacanciesKey } from '@/entities/vacancy';
import { describeError } from '@/shared/api';
import { Button, Callout, TextField, useToast } from '@/shared/ui';
import styles from './refine-form.module.css';

/**
 * The two answers evidence cannot give: total experience, private projects included, and a
 * leading role. Saving re-estimates at once; everything else stays automatic.
 */
export function RefineEstimateForm({ profile }: { profile: Profile }) {
  const scope = useScope();
  const queryClient = useQueryClient();
  const toast = useToast();
  const savedSince = profile.experienceSince === null ? '' : String(profile.experienceSince);
  const typedRole = profile.autoFields.includes('teamRole') ? '' : profile.teamRole;
  const [since, setSince] = useState(savedSince);
  const [role, setRole] = useState(typedRole);
  const [edited, setEdited] = useState(false);
  // Background updates (a review, a re-analysis) never wipe what the candidate is typing; an
  // untouched form follows the saved answers.
  useEffect(() => {
    if (edited) return;
    setSince(savedSince);
    setRole(typedRole);
  }, [edited, savedSince, typedRole]);
  const year = since.length === 4 ? Number(since) : null;
  const thisYear = new Date().getFullYear();
  const validYear = year !== null && year >= 1950 && year <= thisYear;
  const changedSince = validYear && year !== profile.experienceSince;
  const changedRole = role.trim() !== '' && role.trim() !== typedRole;
  const save = useMutation({
    mutationFn: () =>
      updateProfile({
        ...(changedSince ? { experienceSince: year } : {}),
        ...(changedRole ? { teamRole: role.trim() } : {}),
      }),
    onSuccess: (updated) => {
      queryClient.setQueryData(profileKey(scope), updated);
      void queryClient.invalidateQueries({ queryKey: vacanciesKey(scope) });
      setEdited(false);
      toast('Оценка пересчитана');
    },
  });
  return (
    <form
      className={styles.form}
      onSubmit={(event) => {
        event.preventDefault();
        save.mutate();
      }}
    >
      <p className={styles.lead}>
        <strong>Уточните два ответа</strong> — остальное система заполнит сама. Опыт считается с года, когда вы начали в
        IT: учёба, свои проекты и работа в закрытых репозиториях, которых не видно на GitHub. Руководящая роль нужна для
        уровня «Лид».
      </p>
      <div className={styles.fields}>
        <TextField
          id="refine-since"
          label="В IT с какого года"
          inputMode="numeric"
          maxLength={4}
          placeholder="Например: 2016"
          hint={validYear ? `${yearsInIt(year)} лет в IT` : undefined}
          error={since.length === 4 && !validYear ? `Год от 1950 до ${thisYear}` : undefined}
          value={since}
          onChange={(event) => {
            setEdited(true);
            setSince(event.target.value.replace(/\D/g, '').slice(0, 4));
          }}
        />
        <div className={styles.role}>
          <TextField
            id="refine-role"
            label="Роль в команде"
            maxLength={120}
            placeholder="Например: тимлид"
            list="refine-role-suggestions"
            value={role}
            onChange={(event) => {
              setEdited(true);
              setRole(event.target.value);
            }}
          />
        </div>
        <datalist id="refine-role-suggestions">
          <option value="Разработчик" />
          <option value="Ведущий разработчик" />
          <option value="Тимлид" />
          <option value="Архитектор" />
          <option value="Стафф-инженер" />
          <option value="Руководитель направления" />
        </datalist>
      </div>
      <div className="row">
        <Button
          type="submit"
          size="sm"
          variant="primary"
          pending={save.isPending}
          pendingLabel="Пересчитываем…"
          disabled={!profile.consents.processing || (!changedSince && !changedRole)}
        >
          Пересчитать с уточнениями
        </Button>
      </div>
      {save.error && <Callout tone="danger">{describeError(save.error).message}</Callout>}
    </form>
  );
}
