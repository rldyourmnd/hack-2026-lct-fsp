import { useMutation, useQueryClient } from '@tanstack/react-query';
import { type Profile, profileKey, updateProfile } from '@/entities/profile';
import { useScope } from '@/entities/session';
import { describeError } from '@/shared/api';
import { Callout, Checkbox, Switch } from '@/shared/ui';

type Consents = Profile['consents'];

/** Processing and publication are separate decisions; each takes effect only after the server confirms it. */
export function ConsentControls({ profile }: { profile: Profile }) {
  const scope = useScope();
  const queryClient = useQueryClient();
  const save = useMutation({
    mutationFn: (consents: Consents) => updateProfile({ consents }),
    onSuccess: (updated) => queryClient.setQueryData(profileKey(scope), updated),
  });
  const consents = profile.consents;
  return (
    <div className="stack">
      <Checkbox
        checked={consents.processing}
        disabled={save.isPending}
        onChange={(event) => save.mutate({ ...consents, processing: event.target.checked })}
        label="Согласен на обработку данных"
        description="Нужно для обогащения профиля из источников, импорта резюме и проверок. При отзыве незавершённые задачи не применят результат."
      />
      <hr className="divider" />
      <Switch
        id="consent-publication"
        checked={consents.publication}
        disabled={save.isPending}
        onCheckedChange={(publication) => save.mutate({ ...consents, publication })}
        label="Показывать профиль работодателям"
        description="Обезличенный профиль попадёт в поиск компаний, когда у него будет категория — по автоматической оценке или тесту. Контакты при этом не открываются."
      />
      {save.isPending && <span className="small muted">Сохраняем настройку…</span>}
      {save.error && (
        <Callout tone="danger" role="alert">
          Настройка не сохранена: {describeError(save.error).message}
        </Callout>
      )}
    </div>
  );
}
