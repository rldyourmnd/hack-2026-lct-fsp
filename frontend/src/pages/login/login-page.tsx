import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { lazy, Suspense, useState } from 'react';
import { companyQuery } from '@/entities/company';
import { useMeta } from '@/entities/meta';
import { profileQuery } from '@/entities/profile';
import { type Session, scopeOf, sessionQueryOptions } from '@/entities/session';
import { requestEmailCode, useLogout, verifyEmailCode } from '@/features/auth';
import { type components, describeError, isApiError } from '@/shared/api';
import { isDemoStand } from '@/shared/config';
import { pluralRu, useDocumentTitle } from '@/shared/lib';
import { Button, ButtonLink, Callout, Icon, TextField } from '@/shared/ui';
import styles from './login-page.module.css';

type Role = Session['role'];
type Challenge = components['schemas']['EmailChallenge'];

// The local mailbox viewer calls /__mock__ and must not exist in production builds.
const DevMailbox = import.meta.env.DEV ? lazy(() => import('./dev-mailbox')) : null;

const demoAccounts: Array<{ role: Role; email: string; label: string }> = [
  { role: 'candidate', email: 'alexey@example.com', label: 'Кандидат №024 — Алексей Морозов' },
  { role: 'employer', email: 'hr@sever-tech.example', label: 'Работодатель «Север Тех»' },
  { role: 'employer', email: 'hr@second-company.example', label: 'Работодатель «Вторая компания»' },
];
const seededDemoEmailCode = '123456';

const emailPattern = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

function verificationMessage(error: unknown) {
  if (!isApiError(error)) return describeError(error).message;
  switch (error.code) {
    case 'invalid_code':
      return 'Неверный код. Проверьте цифры и попробуйте ещё раз.';
    case 'challenge_expired':
      return 'Срок действия кода истёк. Запросите новый код.';
    case 'challenge_used':
      return 'Этот код уже использован. Запросите новый код.';
    case 'challenge_locked':
      return 'Слишком много неверных попыток. Дождитесь истечения срока кода, затем запросите новый.';
    case 'not_found':
      return 'Запрос подтверждения не найден. Запросите новый код.';
    default:
      return error.message;
  }
}

const needsNewCode = (error: unknown) =>
  isApiError(error) && ['challenge_expired', 'challenge_used', 'challenge_locked', 'not_found'].includes(error.code);

function SignedInNotice({ session }: { session: Session }) {
  const logout = useLogout();
  return (
    <Callout
      tone="info"
      title={`Вы уже вошли: ${session.displayName || (session.role === 'candidate' ? 'кандидат' : 'работодатель')}`}
      actions={
        <>
          <ButtonLink to="/" size="sm" variant="primary">
            Перейти в кабинет
          </ButtonLink>
          <Button size="sm" onClick={() => logout.mutate()} pending={logout.isPending}>
            Выйти
          </Button>
        </>
      }
    >
      Чтобы войти другим аккаунтом, сначала выйдите.
    </Callout>
  );
}

type Props = { redirectTo: string | undefined; expired: boolean };

export function LoginPage({ redirectTo, expired }: Props) {
  useDocumentTitle('Вход и регистрация');
  const meta = useMeta();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const existing = useQuery({ ...sessionQueryOptions(), retry: false });
  const [challenge, setChallenge] = useState<(Challenge & { email: string; role: Role }) | null>(null);
  const [code, setCode] = useState('');

  const requestCode = useMutation({
    mutationFn: ({ email, role }: { email: string; role: Role }) => requestEmailCode(email, role),
    onSuccess: (result, variables) => {
      setChallenge({ ...result, ...variables });
      setCode('');
    },
  });

  const form = useForm({
    defaultValues: { role: 'candidate' as Role, email: '' },
    onSubmit: ({ value }) => requestCode.mutateAsync({ email: value.email.trim(), role: value.role }).catch(() => {}),
  });

  const enterCabinet = async (session: Session) => {
    await openCabinet(session);
    // The sign-in form is gone; start keyboard navigation from the new page's main content.
    document.getElementById('main')?.focus();
  };

  const openCabinet = async (session: Session) => {
    await queryClient.cancelQueries();
    queryClient.clear();
    queryClient.setQueryData(sessionQueryOptions().queryKey, session);
    const scope = scopeOf(session);
    if (session.role === 'candidate') {
      const profile = await queryClient.fetchQuery(profileQuery(scope));
      if (!profile.onboarding.questionnaireComplete) return navigate({ to: '/candidate/onboarding' });
      if (redirectTo?.startsWith('/candidate')) return navigate({ href: redirectTo });
      return navigate({ to: '/candidate/overview' });
    }
    const company = await queryClient.fetchQuery(companyQuery(scope));
    if (!company.readyToInvite) return navigate({ to: '/employer/company' });
    if (redirectTo?.startsWith('/employer')) return navigate({ href: redirectTo });
    return navigate({ to: '/employer/needs' });
  };

  const verify = useMutation({
    mutationFn: () => {
      if (!challenge) throw new Error('Нет активного запроса кода');
      return verifyEmailCode(challenge.challengeId, code);
    },
    onSuccess: enterCabinet,
  });

  const requestError = requestCode.error;
  const roleConflict = isApiError(requestError) && requestError.code === 'role_mismatch';
  const codeValid = /^\d{6}$/.test(code);
  const visibleDemoEmailCode =
    isDemoStand && challenge && demoAccounts.some((account) => account.email === challenge.email)
      ? seededDemoEmailCode
      : null;
  const lifetimeMinutes = challenge
    ? Math.max(0, Math.round((Date.parse(challenge.expiresAt) - Date.parse(meta.serverTime)) / 60_000))
    : 0;

  return (
    <div className={styles.page}>
      <section className={styles.brand} aria-label="О платформе">
        <div className={styles.mark}>
          <span className={styles.markTitle}>ФСП</span>
          <span className={styles.markName}>Карьера</span>
        </div>
        <div className={styles.pitch}>
          <h2 className={styles.pitchTitle}>Работодатель сам выходит на подтверждённого специалиста</h2>
          <ul className={styles.points}>
            <li>
              <Icon name="shield" />
              Категорию для работодателей определяет пройденная проверка, а не самоописание.
            </li>
            <li>
              <Icon name="lock" />
              До вашего согласия компании видят только обезличенный профиль.
            </li>
            <li>
              <Icon name="ruble" />
              Каждое приглашение содержит вилку зарплаты в рублях и контакт компании.
            </li>
          </ul>
        </div>
        <p className={styles.footnote}>Регистрация и вход — по адресу электронной почты с подтверждением кодом.</p>
      </section>

      <main className={styles.panel} id="main">
        <div className="stack">
          <h1 className={styles.title}>{challenge ? 'Подтвердите почту' : 'Вход и регистрация'}</h1>
          <p className="muted">
            {!challenge
              ? 'Новый адрес создаёт аккаунт выбранной роли, существующий — открывает ваш кабинет.'
              : challenge.delivery === 'mock_mailbox'
                ? `Шестизначный код для ${challenge.email} выдан в демонстрационный почтовый ящик.`
                : `Мы отправили шестизначный код на ${challenge.email}.`}
          </p>
        </div>

        {expired && (
          <Callout tone="warning" title="Сессия завершилась">
            Войдите снова, чтобы продолжить работу.
          </Callout>
        )}
        {existing.data && <SignedInNotice session={existing.data} />}

        {!challenge ? (
          <form
            className={styles.form}
            noValidate
            onSubmit={(event) => {
              event.preventDefault();
              void form.handleSubmit();
            }}
          >
            <form.Field name="role">
              {(field) => (
                <fieldset className={styles.roles}>
                  <legend>Роль</legend>
                  {(
                    [
                      {
                        value: 'candidate',
                        title: 'Кандидат',
                        text: 'Подтверждаю уровень и получаю приглашения',
                        icon: 'user',
                      },
                      {
                        value: 'employer',
                        title: 'Работодатель',
                        text: 'Ищу специалистов и отправляю предложения',
                        icon: 'building',
                      },
                    ] as const
                  ).map((option) => (
                    <label key={option.value} className={styles.roleOption}>
                      <input
                        type="radio"
                        name="role"
                        value={option.value}
                        checked={field.state.value === option.value}
                        onChange={() => {
                          field.handleChange(option.value);
                          requestCode.reset();
                        }}
                      />
                      <span className={styles.roleTitle}>
                        <Icon name={option.icon} />
                        {option.title}
                      </span>
                      <span className={styles.roleText}>{option.text}</span>
                    </label>
                  ))}
                </fieldset>
              )}
            </form.Field>
            <form.Field
              name="email"
              validators={{
                onSubmit: ({ value }) =>
                  !value.trim()
                    ? 'Укажите адрес электронной почты'
                    : !emailPattern.test(value.trim())
                      ? 'Проверьте адрес: например, name@example.com'
                      : undefined,
              }}
            >
              {(field) => (
                <TextField
                  id="login-email"
                  label="Электронная почта"
                  type="email"
                  autoComplete="email"
                  required
                  value={field.state.value}
                  onChange={(event) => {
                    field.handleChange(event.target.value);
                    requestCode.reset();
                  }}
                  onBlur={field.handleBlur}
                  error={field.state.meta.errors[0] as string | undefined}
                />
              )}
            </form.Field>
            {requestError && (
              <Callout tone="danger" role="alert" title={roleConflict ? 'Роль аккаунта изменить нельзя' : undefined}>
                {roleConflict
                  ? 'Этот адрес уже зарегистрирован с другой ролью. Выберите другую роль или используйте другой адрес.'
                  : describeError(requestError).message}
              </Callout>
            )}
            <Button
              type="submit"
              variant="primary"
              block
              pending={requestCode.isPending}
              pendingLabel="Отправляем код…"
            >
              Получить код
            </Button>
          </form>
        ) : (
          <form
            className={styles.form}
            noValidate
            onSubmit={(event) => {
              event.preventDefault();
              if (codeValid) verify.mutate();
            }}
          >
            {challenge.delivery === 'mock_mailbox' && (
              <Callout tone="info" title="Демонстрационная почта">
                Письмо на реальный адрес не отправляется: код попадает в демонстрационный почтовый ящик.
                {visibleDemoEmailCode && (
                  <>
                    {' '}
                    Код для этого стенда: <strong>{visibleDemoEmailCode}</strong>.
                  </>
                )}
              </Callout>
            )}
            {challenge.delivery === 'email' && (
              <Callout tone="info" title="Проверьте почту">
                Если письмо не пришло, проверьте папку «Спам» и правильность адреса. Повторная отправка доступна через
                минуту. Никому не сообщайте код.
              </Callout>
            )}
            {DevMailbox && challenge.delivery === 'mock_mailbox' && (
              <Suspense fallback={null}>
                <DevMailbox challengeId={challenge.challengeId} onCode={setCode} />
              </Suspense>
            )}
            <TextField
              id="login-code"
              label="Код из письма"
              className={styles.codeInput}
              inputMode="numeric"
              autoComplete="one-time-code"
              maxLength={6}
              value={code}
              hint={
                lifetimeMinutes > 0
                  ? `Код действует ${lifetimeMinutes} ${pluralRu(lifetimeMinutes, 'минуту', 'минуты', 'минут')}.`
                  : undefined
              }
              error={verify.error ? verificationMessage(verify.error) : undefined}
              onChange={(event) => {
                setCode(event.target.value.replace(/\D/g, '').slice(0, 6));
                verify.reset();
              }}
            />
            <Button
              type="submit"
              variant="primary"
              block
              disabled={!codeValid}
              pending={verify.isPending}
              pendingLabel="Проверяем…"
            >
              Подтвердить и войти
            </Button>
            <div className="row" style={{ justifyContent: 'space-between' }}>
              <Button
                variant={needsNewCode(verify.error) ? 'secondary' : 'quiet'}
                icon="refresh"
                pending={requestCode.isPending}
                onClick={() => {
                  verify.reset();
                  requestCode.mutate({ email: challenge.email, role: challenge.role });
                }}
              >
                Отправить новый код
              </Button>
              <Button
                variant="quiet"
                icon="arrowLeft"
                onClick={() => {
                  setChallenge(null);
                  verify.reset();
                  requestCode.reset();
                }}
              >
                Изменить почту
              </Button>
            </div>
            {requestCode.error && <Callout tone="danger">{describeError(requestCode.error).message}</Callout>}
          </form>
        )}

        {isDemoStand && !challenge && (
          <Callout tone="neutral" title="Демонстрационные аккаунты">
            <p style={{ margin: '0 0 0.5rem' }}>
              Данные стенда синтетические. Для полного пути кандидата укажите любой новый адрес.
            </p>
            <ul className={styles.accounts}>
              {demoAccounts.map((account) => (
                <li key={account.email}>
                  <span>
                    {account.label}
                    <br />
                    <code>{account.email}</code>
                  </span>
                  <Button
                    size="sm"
                    variant="quiet"
                    onClick={() => {
                      form.setFieldValue('role', account.role);
                      form.setFieldValue('email', account.email);
                      requestCode.reset();
                    }}
                    aria-label={`Подставить ${account.email}`}
                  >
                    Подставить
                  </Button>
                </li>
              ))}
            </ul>
          </Callout>
        )}
      </main>
    </div>
  );
}
