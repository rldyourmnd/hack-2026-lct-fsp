import { useSuspenseQuery } from '@tanstack/react-query';
import { RequirementStatusChip } from '@/entities/candidate';
import { companyInitials } from '@/entities/company';
import { skillLabel, useMeta } from '@/entities/meta';
import { profileQuery } from '@/entities/profile';
import { useScope } from '@/entities/session';
import { VacancyFitChip, vacancyQuery } from '@/entities/vacancy';
import { ApplyPanel } from '@/features/apply-vacancy';
import {
  criticalityLabel,
  formatDate,
  gradeLabel,
  specializationLabel,
  useDocumentTitle,
  workModeLabel,
} from '@/shared/lib';
import { Avatar, ButtonLink, Card, Icon, SalaryBlock, TableWrap, Tag } from '@/shared/ui';

export function CandidateVacancyPage({ vacancyId }: { vacancyId: string }) {
  const scope = useScope();
  const meta = useMeta();
  const { data: vacancy } = useSuspenseQuery(vacancyQuery(scope, vacancyId));
  const { data: profile } = useSuspenseQuery(profileQuery(scope));
  useDocumentTitle(vacancy.title);
  const company = vacancy.company;
  const fit = new Map(vacancy.match?.requirements.map((item) => [item.skillId, item]) ?? []);

  return (
    <>
      <ButtonLink to="/candidate/vacancies" variant="quiet" icon="arrowLeft">
        Все вакансии
      </ButtonLink>
      <header className="stack" style={{ margin: '0.75rem 0 1.5rem' }}>
        <h1 style={{ margin: 0, fontSize: '2rem' }}>{vacancy.title}</h1>
        <div className="row">
          <span className="muted">{company.displayName}</span>
          <span className="small muted">· опубликована {formatDate(vacancy.publishedAt)}</span>
        </div>
      </header>
      <div className="split">
        <div className="stack-lg">
          <Card>
            <div className="stack-lg">
              <div className="row" style={{ alignItems: 'flex-start' }}>
                <Avatar kind="company" text={companyInitials(company)} size="lg" />
                <div className="stack" style={{ gap: '0.5rem' }}>
                  <h2 style={{ margin: 0 }}>{company.displayName}</h2>
                  <div className="row">
                    <Tag>
                      <Icon name="pin" size={16} /> {workModeLabel[vacancy.workMode]}
                    </Tag>
                    <Tag>
                      {specializationLabel[vacancy.specialization]} · от {gradeLabel[vacancy.grade]}
                    </Tag>
                  </div>
                </div>
              </div>
              <SalaryBlock salary={vacancy.salary} />
              <hr className="divider" />
              <section className="stack">
                <h3 style={{ margin: 0, fontSize: '1rem' }}>Что предстоит делать</h3>
                <p style={{ whiteSpace: 'pre-wrap' }}>{vacancy.description}</p>
              </section>
              <section className="stack" aria-labelledby="vacancy-requirements">
                <div className="row" style={{ justifyContent: 'space-between' }}>
                  <h3 id="vacancy-requirements" style={{ margin: 0, fontSize: '1rem' }}>
                    Требования
                  </h3>
                  {vacancy.match && <VacancyFitChip status={vacancy.match.status} />}
                </div>
                <TableWrap label="Требования вакансии">
                  <table className="table">
                    <thead>
                      <tr>
                        <th scope="col">Навык</th>
                        <th scope="col">Важность</th>
                        <th scope="col">Минимальный уровень</th>
                        {vacancy.match && <th scope="col">Ваше подтверждение</th>}
                      </tr>
                    </thead>
                    <tbody>
                      {vacancy.requirements.map((item) => {
                        const own = fit.get(item.skillId);
                        return (
                          <tr key={item.skillId}>
                            <td>
                              <strong>{skillLabel(meta, item.skillId)}</strong>
                            </td>
                            <td>{criticalityLabel[item.criticality]}</td>
                            <td>{gradeLabel[item.minimumGrade]}</td>
                            {vacancy.match && <td>{own && <RequirementStatusChip status={own.status} plain />}</td>}
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </TableWrap>
                {vacancy.match ? (
                  <p className="small muted">
                    {vacancy.match.explanation} Соответствие рассчитано сервером по вашему подтверждённому профилю.
                  </p>
                ) : (
                  <p className="small muted">
                    Соответствие появится, когда у профиля будет категория: по GitHub и ФСП или по тесту знаний.
                  </p>
                )}
              </section>
              <hr className="divider" />
              <section className="stack">
                <h3 style={{ margin: 0, fontSize: '1rem' }}>О компании</h3>
                <p>{company.description}</p>
                <p className="small muted">Сфера деятельности: {company.businessArea}</p>
              </section>
              <section className="stack">
                <h3 style={{ margin: 0, fontSize: '1rem' }}>Контакт компании</h3>
                <a href={`mailto:${company.contactEmail}`}>{company.contactEmail}</a>
              </section>
            </div>
          </Card>
        </div>
        <div className="stack-lg">
          <ApplyPanel vacancy={vacancy} profile={profile} />
          <Card tinted title="Отклик — дополнительный путь" headingLevel={3}>
            <p className="small">
              Компании и сами приглашают кандидатов с подтверждённым грейдом, если профиль открыт для показа в
              настройках приватности.
            </p>
          </Card>
        </div>
      </div>
    </>
  );
}
