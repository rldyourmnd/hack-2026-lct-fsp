import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Link, type LinkProps, useNavigate, useRouterState } from '@tanstack/react-router';
import { type ReactNode, useEffect, useRef, useState } from 'react';
import { companyApplicationsQuery } from '@/entities/application';
import { myInvitationsQuery } from '@/entities/invitation';
import { useMeta } from '@/entities/meta';
import { type Session, scopeOf, sessionQueryOptions, useSession } from '@/entities/session';
import { Icon, type IconName } from '@/shared/ui';
import { AccountMenu } from './account-menu';
import styles from './app-shell.module.css';
import { DemoButton } from './demo-dialog';

type NavItem = { to: NonNullable<LinkProps['to']>; label: string; icon: IconName };

const navigation: Record<Session['role'], { cabinet: string; items: NavItem[]; settings: NavItem }> = {
  candidate: {
    cabinet: 'Кабинет кандидата',
    items: [
      { to: '/candidate/overview', label: 'Обзор', icon: 'home' },
      { to: '/candidate/profile', label: 'Профиль', icon: 'user' },
      { to: '/candidate/sources', label: 'Источники', icon: 'inbox' },
      { to: '/candidate/skills', label: 'Навыки', icon: 'chart' },
      { to: '/candidate/assessments', label: 'Проверки', icon: 'shield' },
      { to: '/candidate/tasks', label: 'Задания', icon: 'target' },
      { to: '/candidate/invitations', label: 'Приглашения', icon: 'mail' },
      { to: '/candidate/vacancies', label: 'Вакансии', icon: 'briefcase' },
      { to: '/candidate/applications', label: 'Отклики', icon: 'send' },
    ],
    settings: { to: '/candidate/settings', label: 'Настройки', icon: 'sliders' },
  },
  employer: {
    cabinet: 'Кабинет работодателя',
    items: [
      { to: '/employer/needs', label: 'Потребности', icon: 'briefcase' },
      { to: '/employer/candidates', label: 'Кандидаты', icon: 'users' },
      { to: '/employer/shortlists', label: 'Подборки', icon: 'folder' },
      { to: '/employer/invitations', label: 'Приглашения', icon: 'send' },
      { to: '/employer/applications', label: 'Отклики', icon: 'inbox' },
      { to: '/employer/analytics', label: 'Аналитика', icon: 'chart' },
      { to: '/employer/tasks', label: 'Задания', icon: 'target' },
      { to: '/employer/company', label: 'Компания', icon: 'building' },
    ],
    settings: { to: '/employer/settings', label: 'Настройки', icon: 'sliders' },
  },
};

function CandidateInvitationsLink() {
  const session = useSession();
  const { data } = useQuery(myInvitationsQuery(scopeOf(session)));
  const fresh = data?.items.filter((item) => item.status === 'sent').length ?? 0;
  const label = fresh ? `Приглашения: ${fresh} новых` : 'Приглашения: новых нет';
  return (
    <Link to="/candidate/invitations" className={styles.iconLink} aria-label={label} title={label}>
      <Icon name="bell" size={22} />
      {fresh > 0 && (
        <span className={styles.badge} aria-hidden="true">
          {fresh}
        </span>
      )}
    </Link>
  );
}

function EmployerApplicationsLink() {
  const session = useSession();
  const { data } = useQuery(companyApplicationsQuery(scopeOf(session)));
  const fresh = data?.items.filter((item) => item.status === 'sent').length ?? 0;
  const label = fresh ? `Отклики: ${fresh} новых` : 'Отклики: новых нет';
  return (
    <Link to="/employer/applications" className={styles.iconLink} aria-label={label} title={label}>
      <Icon name="bell" size={22} />
      {fresh > 0 && (
        <span className={styles.badge} aria-hidden="true">
          {fresh}
        </span>
      )}
    </Link>
  );
}

/** Detects a different account signed in from another tab and drops this tab's private cache. */
function useActorChangeGuard(actorId: string) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const { data } = useQuery({ ...sessionQueryOptions(), refetchOnWindowFocus: 'always' });
  useEffect(() => {
    if (data && data.actorId !== actorId) {
      queryClient.clear();
      void navigate({ to: '/' });
    }
  }, [data, actorId, queryClient, navigate]);
}

export function AppShell({ children }: { children: ReactNode }) {
  const session = useSession();
  const meta = useMeta();
  const config = navigation[session.role];
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const [drawerOpen, setDrawerOpen] = useState(false);
  const mainRef = useRef<HTMLElement>(null);
  const shownPath = useRef(pathname);
  useActorChangeGuard(session.actorId);

  const section = [...config.items, config.settings].find((item) => pathname.startsWith(String(item.to)));

  useEffect(() => {
    if (shownPath.current === pathname) return;
    shownPath.current = pathname;
    setDrawerOpen(false);
    // Move focus with the view so keyboard and screen-reader users are not left on a stale control.
    mainRef.current?.focus({ preventScroll: true });
  }, [pathname]);

  useEffect(() => {
    if (!drawerOpen) return;
    const onKey = (event: KeyboardEvent) => event.key === 'Escape' && setDrawerOpen(false);
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [drawerOpen]);

  const renderLink = (item: NavItem) => (
    <Link to={item.to} className={styles.navLink}>
      <Icon name={item.icon} size={22} />
      {item.label}
    </Link>
  );

  return (
    <div className={styles.shell}>
      <a href="#main" className={styles.skip}>
        Перейти к содержимому
      </a>
      <button
        type="button"
        className={styles.backdrop}
        data-open={drawerOpen}
        aria-label="Закрыть меню"
        tabIndex={-1}
        onClick={() => setDrawerOpen(false)}
      />
      <aside className={styles.sidebar} data-open={drawerOpen} id="app-navigation">
        <Link to="/" className={styles.brand} aria-label="ФСП / Карьера — на главную">
          <span className={styles.brandMark}>ФСП</span>
          <span className={styles.brandName}>Карьера</span>
        </Link>
        <nav aria-label={config.cabinet}>
          <ul className={styles.nav}>
            {config.items.map((item) => (
              <li key={String(item.to)}>{renderLink(item)}</li>
            ))}
          </ul>
        </nav>
        <div className={styles.navBottom}>{renderLink(config.settings)}</div>
      </aside>
      <div className={styles.column}>
        <header className={styles.header}>
          <button
            type="button"
            className={styles.menuToggle}
            aria-controls="app-navigation"
            aria-expanded={drawerOpen}
            aria-label="Открыть меню"
            onClick={() => setDrawerOpen(true)}
          >
            <Icon name="menu" size={24} />
          </button>
          <nav aria-label="Навигационная цепочка" className={styles.crumbsNav}>
            <ol className={styles.crumbs}>
              <li>{session.role === 'employer' ? `${config.cabinet} · ${session.displayName}` : config.cabinet}</li>
              {section && <li aria-current="page">{section.label}</li>}
            </ol>
          </nav>
          <div className={styles.headerActions}>
            {meta.mode === 'mock' && <DemoButton />}
            {session.role === 'candidate' ? <CandidateInvitationsLink /> : <EmployerApplicationsLink />}
            <AccountMenu session={session} settingsTo={config.settings.to} />
          </div>
        </header>
        <main className={styles.main} id="main" ref={mainRef} tabIndex={-1}>
          {children}
        </main>
      </div>
    </div>
  );
}
