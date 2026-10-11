import { useForm } from '@tanstack/react-form';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useId } from 'react';
import { useMeta } from '@/entities/meta';
import { profileKey } from '@/entities/profile';
import { useScope } from '@/entities/session';
import { graphKey, skillsKey } from '@/entities/skill';
import { authorizeGitHub, connectSource, type Source, sourcesKey } from '@/entities/source';
import { describeError } from '@/shared/api';
import { Button, Callout, TextField } from '@/shared/ui';

type Props = {
  provider: Source['provider'];
  label: string;
  submitLabel: string;
  hint?: string;
  placeholder?: string;
  onConnected?: () => void;
};

export function ConnectSourceForm({ provider, label, submitLabel, hint, placeholder, onConnected }: Props) {
  const id = useId();
  const scope = useScope();
  const meta = useMeta();
  const queryClient = useQueryClient();
  const connect = useMutation({
    mutationFn: (account: string) => connectSource(provider, account),
    // A source that completes at once also updates the evidence, the estimate and the profile.
    onSuccess: async () => {
      await Promise.all(
        [sourcesKey(scope), skillsKey(scope), graphKey(scope), profileKey(scope)].map((queryKey) =>
          queryClient.invalidateQueries({ queryKey }),
        ),
      );
      onConnected?.();
    },
  });
  const authorization = useMutation({
    mutationFn: authorizeGitHub,
    onSuccess: ({ authorizationUrl }) => window.location.assign(authorizationUrl),
  });
  const form = useForm({
    defaultValues: { account: '' },
    onSubmit: ({ value }) => connect.mutateAsync(value.account.trim()).catch(() => {}),
  });
  const available =
    provider !== 'github' ||
    meta.capabilities.githubInsights ||
    meta.capabilities.githubOAuth ||
    (meta.mode === 'mock' && meta.capabilities.githubFixture);
  if (!available) return <Callout tone="neutral">Анализ GitHub сейчас не настроен на сервере.</Callout>;
  return (
    <form
      className="stack"
      noValidate
      onSubmit={(event) => {
        event.preventDefault();
        void form.handleSubmit();
      }}
    >
      <form.Field
        name="account"
        validators={{ onSubmit: ({ value }) => (value.trim() ? undefined : 'Укажите идентификатор аккаунта') }}
      >
        {(field) => (
          <TextField
            id={`${id}-account`}
            label={label}
            maxLength={100}
            placeholder={placeholder}
            hint={hint}
            value={field.state.value}
            onChange={(event) => field.handleChange(event.target.value)}
            error={field.state.meta.errors[0] as string | undefined}
          />
        )}
      </form.Field>
      {(connect.error || authorization.error) && (
        <Callout tone="danger" role="alert">
          {describeError(connect.error ?? authorization.error).message}
        </Callout>
      )}
      {provider === 'github' && meta.mode === 'mock' && !meta.capabilities.githubInsights && (
        <Callout tone="info">
          Демонстрационное подключение: данные вымышленные, владение аккаунтом не проверяется.
        </Callout>
      )}
      {provider === 'github' && meta.capabilities.githubOAuth && (
        <Button
          type="button"
          variant="secondary"
          pending={authorization.isPending}
          pendingLabel="Переходим в GitHub…"
          onClick={() => authorization.mutate()}
        >
          Подтвердить владение через GitHub
        </Button>
      )}
      <Button
        type="submit"
        variant="primary"
        disabled={!available}
        pending={connect.isPending}
        pendingLabel="Подключаем…"
      >
        {submitLabel}
      </Button>
    </form>
  );
}
