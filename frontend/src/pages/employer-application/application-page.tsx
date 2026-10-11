import { useMutation, useQuery, useQueryClient, useSuspenseQuery } from '@tanstack/react-query';
import { useEffect, useRef } from 'react';
import {
  ApplicationStatusChip,
  applicationsKey,
  companyApplicationQuery,
  markApplicationViewed,
} from '@/entities/application';
import { candidateQuery, disclosureQuery, RequirementStatusChip } from '@/entities/candidate';
import { skillLabel, useMeta } from '@/entities/meta';
import { CategoryBadge } from '@/entities/qualification';
import { useScope } from '@/entities/session';
import { isDemoReference } from '@/entities/source';
import { ApplicationReview } from '@/features/review-application';
import {
  criticalityLabel,
  formatDateTime,
  gradeLabel,
  specializationLabel,
  useDocumentTitle,
  workModeLabel,
} from '@/shared/lib';
import { Avatar, ButtonLink, Card, Icon, LoadingState, SalaryBlock, StatusChip, TableWrap } from '@/shared/ui';

function Contacts({ candidateId }: { candidateId: string }) {
  const scope = useScope();
  const disclosure = useQuery(disclosureQuery(scope, candidateId));
  if (disclosure.isPending) return <LoadingState label="Загружаем контакты…" lines={2} />;
  const grant = disclosure.data;
  if (!grant?.granted) {
    return (
      <p className="small muted" style={{ display: 'flex', gap: '0.5rem' }}>
        <Icon name="lock" size={18} />
        Контакты закрыты: кандидат отозвал отклик или не открывал их вашей компании.
      </p>
    );
  }
  const { fields } = grant;
  return (
    <div className="stack">
      {fields.name && <strong>{fields.name}</strong>}
      {fields.email && <a href={`mailto:${fields.email}`}>{fields.email}</a>}
      {fields.sourceLinks && fields.sourceLinks.length > 0 && (
        <ul className="small" style={{ margin: 0, paddingLeft: '1.25rem' }}>
          {fields.sourceLinks.map((link) => (
            <li key={link}>
              {isDemoReference(link) ? (
                `${link.replace('https://example.com/fixtures/', '')} (демо-ссылка)`
              ) : (
                <a href={link} target="_blank" rel="noreferrer noopener">
                  {link}
                </a>
              )}
            </li>
          ))}
        </ul>
      )}
      <p className="small muted">Доступно только вашей компании и только выбранные кандидатом поля.</p>
    </div>
  );
}

export function EmployerApplicationPage({ applicationId }: { applicationId: string }) {
  useDocumentTitle('Отклик');
  const scope = useScope();
  const meta = useMeta();
  const queryClient = useQueryClient();
  const { data: application } = useSuspenseQuery(companyApplicationQuery(scope, applicationId));
  const candidate = useQuery({
    ...candidateQuery(scope, application.candidateId),
    enabled: application.status !== 'withdrawn',
  });
  const viewed = useRef(false);
  const markViewed = useMutation({
    mutationFn: () => markApplicationViewed(applicationId),
    onSuccess: (updated) => {
      queryClient.setQueryData([...applicationsKey(scope), applicationId], updated);
      void queryClient.invalidateQueries({ queryKey: applicationsKey(scope), exact: true });
    },
  });
  const { mutate } = markViewed;
  useEffect(() => {
    // Opening the application records "viewed" once; the candidate sees it in their status.
    if (application.status === 'sent' && !viewed.current) {
      viewed.current = true;
      mutate();
    }
  }, [application.status, mutate]);

  const { vacancy, match } = application;
  return (
    <>
      <ButtonLink to="/employer/applications" variant="quiet" icon="arrowLeft">
        Все отклики
      </ButtonLink>
      <header className="stack" style={{ margin: '0.75rem 0 1.5rem' }}>
        <h1 style={{ margin: 0, fontSize: '2rem' }}>Отклик: {application.candidateAlias}</h1>
        <div className="row">
          <ApplicationStatusChip status={application.status} audience="employer" />
          <span className="small muted">Получен {formatDateTime(application.createdAt)}</span>
        </div>
      </header>
      <div className="split">
        <div className="stack-lg">
          <Card title="Кандидат">
            <div className="stack">
              <div className="row" style={{ alignItems: 'flex-start' }}>
                <Avatar kind="anonymous" size="lg" />
                <div className="stack" style={{ gap: '0.5rem' }}>
                  <strong>{application.candidateAlias}</strong>
                  {candidate.data && (
                    <div className="row">
                      <CategoryBadge
                        specialization={candidate.data.specialization}
                        grade={candidate.data.grade}
                        source={candidate.data.categorySource}
                        confidence={candidate.data.categoryConfidence}
                      />
                      {candidate.data.fspVerified && (
                        <StatusChip tone="info" icon="checkCircle">
                          Участие в ФСП подтверждено
                        </StatusChip>
                      )}
                    </div>
                  )}
                </div>
              </div>
              {application.status !== 'withdrawn' && (
                <ButtonLink
                  to="/employer/candidates/$candidateId"
                  params={{ candidateId: application.candidateId }}
                  icon="user"
                  block
                >
                  Полный профиль и граф навыков
                </ButtonLink>
              )}
              <section className="stack" aria-labelledby="application-contacts">
                <h3 id="application-contacts" style={{ margin: 0, fontSize: '1rem' }}>
                  Контакты, открытые откликом
                </h3>
                <Contacts candidateId={application.candidateId} />
              </section>
            </div>
          </Card>
          <Card title="Письмо кандидата">
            <p style={{ whiteSpace: 'pre-wrap' }}>{application.message}</p>
          </Card>
          <Card title="Соответствие вакансии">
            <div className="stack">
              <p>{match.explanation}</p>
              <TableWrap label="Соответствие требованиям вакансии">
                <table className="table">
                  <thead>
                    <tr>
                      <th scope="col">Навык</th>
                      <th scope="col">Важность</th>
                      <th scope="col">Подтверждение</th>
                    </tr>
                  </thead>
                  <tbody>
                    {match.requirements.map((item) => (
                      <tr key={item.skillId}>
                        <td>
                          <strong>{skillLabel(meta, item.skillId)}</strong>
                          <div className="small muted">{item.reason}</div>
                        </td>
                        <td>{criticalityLabel[item.criticality]}</td>
                        <td>
                          <RequirementStatusChip status={item.status} plain />
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </TableWrap>
              <p className="small muted">
                Рассчитано сервером по подтверждённому профилю и требованиям на момент отклика ({match.algorithmVersion}
                ).
              </p>
            </div>
          </Card>
        </div>
        <div className="stack-lg">
          <ApplicationReview application={application} />
          <Card title="Условия вакансии">
            <div className="stack">
              <strong>{vacancy.title}</strong>
              <span className="small muted">
                {specializationLabel[vacancy.specialization]} · от {gradeLabel[vacancy.grade]} ·{' '}
                {workModeLabel[vacancy.workMode]}
              </span>
              <SalaryBlock salary={vacancy.salary} />
              <p className="small muted">Зафиксированы на момент отклика.</p>
            </div>
          </Card>
        </div>
      </div>
    </>
  );
}
