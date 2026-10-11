import { useQuery, useSuspenseQuery } from '@tanstack/react-query';
import { type ReactNode, useState } from 'react';
import { useMeta } from '@/entities/meta';
import { eligibilityReasonText, type Profile, profileQuery } from '@/entities/profile';
import { useScope } from '@/entities/session';
import { mySourcesQuery, type Source } from '@/entities/source';
import { ConnectSourceForm, useSourceProcessing } from '@/features/connect-source';
import { ConsentControls } from '@/features/consents';
import { ProfileForm } from '@/features/edit-profile';
import { ImportProfileDialog } from '@/features/profile-pdf';
import { useStartAssessment } from '@/features/start-assessment';
import { describeError } from '@/shared/api';
import { gradeLabel, specializationLabel, useDocumentTitle } from '@/shared/lib';
import { Button, ButtonLink, Callout, Card, Icon, InlineStatus, PageHeader } from '@/shared/ui';
import { AnalysisProgress } from '@/widgets/github-insights';
import { ProfileEstimateCard } from '@/widgets/profile-estimate';
import styles from './onboarding-page.module.css';

function Step({ index, done, title, children }: { index: number; done: boolean; title: string; children: ReactNode }) {
  return (
    <li className={styles.step}>
      <span className={`${styles.marker} ${done ? styles.done : ''}`} aria-hidden="true">
        {done ? <Icon name="check" /> : index}
      </span>
      <Card title={title} subtitle={done ? 'Готово' : undefined}>
        {children}
      </Card>
    </li>
  );
}

/** GitHub with its running analysis; polling the job applies the report and the estimate. */
function GithubSource({ source, consent }: { source: Source; consent: boolean }) {
  const processing = useSourceProcessing(source);
  if (source.status !== 'connected') {
    return consent ? (
      <ConnectSourceForm
        provider="github"
        label="GitHub: ник или ссылка на профиль"
        placeholder="octocat или https://github.com/octocat"
        submitLabel="Подключить GitHub"
        hint="Анализ публичной работы идёт на нашем сервере несколько минут, токен не нужен."
      />
    ) : null;
  }
  return (
    <div className="stack" style={{ gap: '0.5rem' }}>
      <p className="row" style={{ margin: 0 }}>
        <Icon name={processing.processing ? 'refresh' : 'checkCircle'} />
        GitHub «{source.accountLabel}» подключён{processing.processing ? '' : ', анализ завершён'}.
      </p>
      {processing.processing &&
        (source.github?.analysis ? (
          <AnalysisProgress analysis={source.github.analysis} />
        ) : (
          <InlineStatus>Анализ GitHub продолжается…</InlineStatus>
        ))}
      {processing.lastFailure && <Callout tone="warning">{processing.lastFailure.message}</Callout>}
    </div>
  );
}

function FspSource({ source, consent }: { source: Source; consent: boolean }) {
  const processing = useSourceProcessing(source);
  if (source.status !== 'connected') {
    return consent ? (
      <ConnectSourceForm
        provider="fsp"
        label="ФСП ID участника соревнований"
        placeholder="FSP-DEMO-024"
        submitLabel="Подключить ФСП"
        hint="Демонстрационный сервис с вымышленными данными: FSP-DEMO-024 — командный результат, FSP-DEMO-031 — история побед в командном и личном зачёте."
      />
    ) : null;
  }
  const published = (source.fspResults ?? []).filter((result) => result.status === 'published').length;
  return (
    <p className="row" style={{ margin: 0 }}>
      <Icon name={processing.processing ? 'refresh' : 'checkCircle'} />
      ФСП «{source.accountLabel}» подключена
      {processing.processing ? ', ищем результаты…' : ` · опубликованных результатов: ${published}`}.
    </p>
  );
}

function SourcesStep({ profile }: { profile: Profile }) {
  const scope = useScope();
  // A running analysis reports its stage with the source list.
  const sources = useQuery({
    ...mySourcesQuery(scope),
    refetchInterval: (query) => (query.state.data?.items.some((item) => item.github?.analysis) ? 3_000 : false),
  });
  const github = sources.data?.items.find((item) => item.provider === 'github');
  const fsp = sources.data?.items.find((item) => item.provider === 'fsp');
  const consent = profile.consents.processing;
  return (
    <div className="stack">
      <p className="muted" style={{ margin: 0 }}>
        Достаточно одного источника. По публичной работе на GitHub и результатам соревнований ФСП система заполнит
        анкету, определит специализацию и оценит грейд. Анализируются только открытые данные.
      </p>
      {!consent && (
        <Callout tone="neutral">Подключение источников доступно после согласия на обработку данных.</Callout>
      )}
      {sources.isError && (
        <Callout tone="warning">Не удалось загрузить источники. Подключить их можно на странице источников.</Callout>
      )}
      <div className={styles.sources}>
        {github && <GithubSource source={github} consent={consent} />}
        {fsp && <FspSource source={fsp} consent={consent} />}
      </div>
      <div>
        <ButtonLink to="/candidate/sources" iconAfter="arrowRight">
          Подробный отчёт и подтверждение владения
        </ButtonLink>
      </div>
    </div>
  );
}

function AssessmentStep({ profile }: { profile: Profile }) {
  const start = useStartAssessment();
  const eligibility = profile.assessmentEligibility;
  if (profile.onboarding.assessmentPassed) {
    return (
      <div className="row">
        <p>Категория подтверждена тестом.</p>
        <ButtonLink to="/candidate/overview" variant="primary">
          Открыть профиль
        </ButtonLink>
      </div>
    );
  }
  return (
    <div className="stack">
      <p className="muted" style={{ margin: 0 }}>
        Тест знаний необязателен. Он подтверждает категорию: подтверждённые кандидаты стоят выше в выдаче, а результат
        заменяет автоматическую оценку. Подтверждённый грейд не понижается.
      </p>
      {eligibility.canStartSelected ? (
        <>
          <p style={{ margin: 0 }}>
            Цель теста: {specializationLabel[eligibility.selectedSpecialization]} ·{' '}
            {gradeLabel[eligibility.selectedGrade]}. ИИ разрешён на всех этапах.
          </p>
          <div className="row">
            <Button
              variant="primary"
              iconAfter="arrowRight"
              pending={start.isPending}
              onClick={() =>
                start.mutate({ specialization: eligibility.selectedSpecialization, grade: eligibility.selectedGrade })
              }
            >
              Пройти тест
            </Button>
            <ButtonLink to="/candidate/assessments">Как устроен тест</ButtonLink>
          </div>
          {start.error && <Callout tone="danger">{describeError(start.error).message}</Callout>}
        </>
      ) : (
        <Callout tone="neutral">
          {eligibility.selectedReason ? eligibilityReasonText[eligibility.selectedReason] : 'Тест пока недоступен.'}
        </Callout>
      )}
    </div>
  );
}

export function CandidateOnboardingPage() {
  useDocumentTitle('Анкета кандидата');
  const scope = useScope();
  const meta = useMeta();
  const { data: profile } = useSuspenseQuery(profileQuery(scope));
  const sources = useQuery(mySourcesQuery(scope));
  const [importOpen, setImportOpen] = useState(false);
  const form = profile.onboarding;
  // Open while the questionnaire is incomplete; saving it does not collapse the form.
  const [questionnaireOpen, setQuestionnaireOpen] = useState(!form.questionnaireComplete);
  const finished = (sources.data?.items ?? []).some((item) => item.status === 'connected' && !item.activeJobId);
  const category = profile.category.source !== null;

  return (
    <>
      <PageHeader
        title="Анкета кандидата"
        subtitle="Подключите GitHub или ФСП — система сама заполнит анкету и оценит специализацию и грейд. Править ничего не обязательно; тест по желанию подтверждает категорию."
      />
      <ol className={styles.steps}>
        <Step index={1} done={form.emailVerified} title="Почта подтверждена">
          <p className="muted">
            {profile.email}. Адрес используется для входа; работодатели увидят его, только если вы сами откроете
            контакты в ответ на приглашение.
          </p>
        </Step>
        <Step index={2} done={profile.consents.processing} title="Согласия">
          <ConsentControls profile={profile} />
        </Step>
        <Step index={3} done={Boolean(profile.estimate) || finished} title="Подключите GitHub или ФСП">
          <SourcesStep profile={profile} />
        </Step>
        <Step index={4} done={category && form.questionnaireComplete} title="Профиль собран автоматически">
          <div className="stack">
            {profile.estimate || finished ? (
              <ProfileEstimateCard profile={profile} canEstimate={finished} title="Оценка по открытым данным" />
            ) : (
              <p className="muted" style={{ margin: 0 }}>
                Оценка появится, когда подключится источник из шага 3. Без источников анкету можно заполнить вручную, а
                категорию получить по тесту.
              </p>
            )}
            <details
              className={styles.details}
              open={questionnaireOpen}
              onToggle={(event) => setQuestionnaireOpen(event.currentTarget.open)}
            >
              <summary>
                {form.questionnaireComplete ? 'Проверить и поправить анкету' : 'Заполнить анкету вручную'}
              </summary>
              <div className="stack" style={{ marginTop: '1rem' }}>
                <div className="row" style={{ justifyContent: 'space-between' }}>
                  <p className="muted" style={{ margin: 0 }}>
                    Можно поправить любое поле или загрузить резюме в PDF.
                  </p>
                  {meta.capabilities.profilePdfImport && (
                    <Button icon="upload" onClick={() => setImportOpen(true)} disabled={!profile.consents.processing}>
                      Заполнить из PDF
                    </Button>
                  )}
                </div>
                <ProfileForm profile={profile} submitLabel="Сохранить анкету" />
              </div>
            </details>
          </div>
        </Step>
        <Step index={5} done={form.assessmentPassed} title="Тест знаний — по желанию">
          <AssessmentStep profile={profile} />
        </Step>
      </ol>
      <ImportProfileDialog open={importOpen} onClose={() => setImportOpen(false)} />
    </>
  );
}
