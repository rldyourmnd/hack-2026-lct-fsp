import type { RumEvent, RumEventDomainContext } from '@openobserve/browser-rum';

const actionNames = new Map([
  ['Откликнуться и открыть контакты', 'apply_vacancy'],
  ['Принять и открыть контакты', 'accept_invitation'],
  ['Подтвердить и войти', 'sign_in'],
  ['Получить код', 'request_login_code'],
  ['Сохранить', 'save'],
  ['Сохранить изменения', 'save'],
  ['Опубликовать', 'publish_vacancy'],
  ['Отклонить', 'decline'],
  ['Отозвать', 'withdraw'],
  ['Отправить ответ', 'submit_answer'],
  ['Начать проверку', 'start_assessment'],
  ['Обновить оценку', 'refresh_estimate'],
  ['Синхронизировать', 'sync_source'],
  ['Выйти', 'sign_out'],
]);

const routeWords = new Set(
  'api v1 me auth email challenges verify logout session meta candidate employer profile sources skills assessments assessment vacancies applications invitations tasks needs candidates shortlists settings onboarding dashboard analytics graphs graph knowledge results result workspace interview interviews answers speech estimate refine refresh sync github fsp connect disconnect ownership jobs notifications read publish unpublish accept decline withdraw disclosure gap-plan import confirm reports health assets login'.split(
    ' ',
  ),
);
const needId = /^need-(?:[0-9a-f]{32}|(?:a-|b-)?\d{1,6})$/;
const resourceId =
  /^(?:candidate-[0-9a-f]{12}|candidate\d{3,6}|(?:need|application|invitation|shortlist|task|job)-(?:[0-9a-f]{32}|(?:a-|b-)?\d{1,6}))$/;

/** Never retain a query, fragment, account handle, object identifier or third-party URL. */
export function safeUrl(raw: string): string {
  try {
    const url = new URL(raw, window.location.origin);
    if (url.origin !== window.location.origin) return `${window.location.origin}/external`;
    const route = url.pathname.split('/').map((part) => (part === '' || routeWords.has(part) ? part : ':id'));
    return `${url.origin}${route.join('/')}`;
  } catch {
    return `${window.location.origin}/unknown`;
  }
}

/** Replay metadata contains the real URL and cannot be sanitized through beforeSend. */
export function canReplay(raw: string): boolean {
  const url = new URL(raw, window.location.origin);
  if (url.origin !== window.location.origin || url.hash) return false;
  if (!/^\/(candidate|employer)(\/|$)/.test(url.pathname)) return false;
  if (/\/(settings|onboarding|interviews|assessments)(\/|$)/.test(url.pathname)) return false;
  for (const [key, value] of url.searchParams) {
    // Only the application's opaque vacancy reference is needed for the candidate detail screen.
    if (key !== 'needId' || !needId.test(value)) return false;
  }
  return url.pathname.split('/').every((part) => part === '' || routeWords.has(part) || resourceId.test(part));
}

export function sanitizeEvent(event: RumEvent, context?: RumEventDomainContext): boolean {
  event.view.url = safeUrl(event.view.url);
  event.view.name = new URL(event.view.url).pathname;
  event.view.referrer = '';
  event.context = {
    audience: event.view.name.startsWith('/employer')
      ? 'employer'
      : event.view.name.startsWith('/candidate')
        ? 'candidate'
        : 'anonymous',
  };
  if (event.type === 'error') {
    // Causes and custom error payloads are not editable in the pinned SDK.
    if (event.error.causes?.length) return false;
    event.error.message = 'Browser error';
    event.error.stack = '';
    event.error.handling_stack = '';
    event.error.fingerprint = 'browser-error';
    if (event.error.resource) event.error.resource.url = safeUrl(event.error.resource.url);
  } else if (event.type === 'resource') {
    event.resource.url = safeUrl(event.resource.url);
    if (
      event.resource.url === `${window.location.origin}/external` ||
      event.resource.url === `${window.location.origin}/unknown`
    )
      return false;
    if (event.resource.graphql) event.resource.graphql.variables = '';
    if (event.resource.request) event.resource.request.headers = {};
    if (event.resource.response) event.resource.response.headers = {};
  } else if (event.type === 'action' && event.action.target) {
    event.action.target.name = event.action.type;
    const target = context && 'events' in context ? context.events?.[0]?.target : null;
    const element = target instanceof Element ? target.closest('a,button') : null;
    if (element instanceof HTMLAnchorElement)
      event.action.target.name = `navigate:${new URL(safeUrl(element.href)).pathname}`;
    else if (element) {
      const label = element.textContent?.trim().replace(/\s+/g, ' ') ?? '';
      event.action.target.name = actionNames.get(label) ?? event.action.type;
    }
  } else if (event.type === 'long_task') {
    for (const script of event.long_task.scripts ?? []) {
      script.source_url = safeUrl(script.source_url ?? '');
      script.invoker = '';
    }
  } else if (event.type === 'view' && event.view.performance?.lcp?.resource_url) {
    event.view.performance.lcp.resource_url = safeUrl(event.view.performance.lcp.resource_url);
  }
  return true;
}
