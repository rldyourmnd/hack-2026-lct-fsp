import { useSuspenseQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { type Invitation, InvitationStatusChip, myInvitationsQuery } from '@/entities/invitation';
import { useScope } from '@/entities/session';
import { formatDate, formatSalaryRange, taxBasisLabel, useDocumentTitle, workModeLabel } from '@/shared/lib';
import { Card, EmptyState, PageHeader, StatusChip, type TabItem, Tabs } from '@/shared/ui';

export type InvitationFilter = 'all' | 'open' | 'accepted' | 'closed';

const matches: Record<InvitationFilter, (invitation: Invitation) => boolean> = {
  all: () => true,
  open: (invitation) => invitation.status === 'sent' || invitation.status === 'viewed',
  accepted: (invitation) => invitation.status === 'accepted',
  closed: (invitation) => invitation.status === 'declined' || invitation.status === 'withdrawn',
};

type Props = { filter: InvitationFilter; onFilterChange: (filter: InvitationFilter) => void };

export function CandidateInvitationsPage({ filter, onFilterChange }: Props) {
  useDocumentTitle('Приглашения');
  const scope = useScope();
  const { data } = useSuspenseQuery({ ...myInvitationsQuery(scope), refetchOnWindowFocus: 'always' });
  const items = [...data.items].sort((a, b) => b.createdAt.localeCompare(a.createdAt));
  const tabs: TabItem<InvitationFilter>[] = [
    { id: 'all', label: 'Все', count: items.length },
    { id: 'open', label: 'Ждут решения', count: items.filter(matches.open).length },
    { id: 'accepted', label: 'Принятые', count: items.filter(matches.accepted).length },
    { id: 'closed', label: 'Отклонённые и отозванные', count: items.filter(matches.closed).length },
  ];
  const visible = items.filter(matches[filter]);
  return (
    <>
      <PageHeader
        title="Приглашения"
        subtitle="Компании сами выходят на вас: условия и контакт работодателя видны до решения"
      />
      <Card>
        <Tabs items={tabs} value={filter} onChange={onFilterChange} label="Статус приглашений" idPrefix="invitations" />
        <div role="tabpanel" id={`invitations-panel-${filter}`} aria-labelledby={`invitations-tab-${filter}`}>
          {visible.length === 0 ? (
            <EmptyState icon="mail" title={items.length ? 'В этом разделе пусто' : 'Приглашений пока нет'}>
              {items.length
                ? 'Выберите другой статус.'
                : 'Работодатели видят обезличенный профиль, когда у него есть категория и вы согласились на показ. Приглашение придёт сюда.'}
            </EmptyState>
          ) : (
            <ul className="stack" style={{ margin: '1rem 0 0', padding: 0, listStyle: 'none' }}>
              {visible.map((invitation) => (
                <li key={invitation.id}>
                  <Card as="article" headingLevel={3}>
                    <div className="row" style={{ justifyContent: 'space-between', alignItems: 'flex-start' }}>
                      <div className="stack" style={{ gap: '0.25rem' }}>
                        <h3 style={{ margin: 0 }}>
                          <Link to="/candidate/invitations/$invitationId" params={{ invitationId: invitation.id }}>
                            {invitation.company.displayName}: {invitation.need.title}
                          </Link>
                        </h3>
                        <span>
                          <strong>{formatSalaryRange(invitation.need.salary)}</strong>{' '}
                          <span className="small muted">{taxBasisLabel[invitation.need.salary.taxBasis]}</span>
                        </span>
                        <span className="small muted">
                          {workModeLabel[invitation.need.workMode]} · получено {formatDate(invitation.createdAt)}
                        </span>
                      </div>
                      <div className="row">
                        {invitation.status === 'sent' && (
                          <StatusChip tone="accent" icon={null}>
                            Новое
                          </StatusChip>
                        )}
                        <InvitationStatusChip status={invitation.status} audience="candidate" />
                      </div>
                    </div>
                  </Card>
                </li>
              ))}
            </ul>
          )}
        </div>
      </Card>
    </>
  );
}
