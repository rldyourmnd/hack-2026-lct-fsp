import type { components } from './generated/schema.d.ts';

type Result = components['schemas']['FspResult'];

const published = (
  competitionId: string,
  competitionName: string,
  discipline: string,
  [startsAt, endsAt, publishedAt]: [string, string, string],
  team: boolean,
  [place, fieldSize, score]: [number, number, number],
  protocolReference: string,
): Result => ({
  competitionId,
  competitionName,
  discipline,
  startsAt,
  endsAt,
  scope: team ? 'team' : 'individual',
  teamName: team ? 'Команда Пример' : null,
  teamSize: team ? 4 : null,
  status: 'published',
  place,
  fieldSize,
  score,
  protocolReference,
  publishedAt,
  synthetic: true,
});

/** The longer fictional history of `FSP-DEMO-031`, as the Rust simulator serves it. */
const HISTORY_031: Result[] = [
  published(
    'competition-demo-platform-2024',
    'Демо-хакатон «Платформа» 2024 (вымышленное событие)',
    'Продуктовое программирование',
    ['2024-11-15T09:00:00Z', '2024-11-17T18:00:00Z', '2024-11-20T12:00:00Z'],
    true,
    [1, 64, 97],
    'DEMO-PROTOCOL-PLATFORM-2024',
  ),
  published(
    'competition-demo-product-2025',
    'Демо-чемпионат по продуктовому программированию 2025 (вымышленное событие)',
    'Продуктовое программирование',
    ['2025-10-03T09:00:00Z', '2025-10-05T18:00:00Z', '2025-10-08T12:00:00Z'],
    true,
    [2, 120, 94],
    'DEMO-PROTOCOL-PRODUCT-2025',
  ),
  published(
    'competition-demo-algorithms-2025',
    'Демо-кубок по алгоритмическому программированию 2025 (вымышленное событие)',
    'Алгоритмическое программирование',
    ['2025-04-12T09:00:00Z', '2025-04-12T14:00:00Z', '2025-04-14T12:00:00Z'],
    false,
    [9, 412, 88],
    'DEMO-PROTOCOL-ALGORITHMS-2025',
  ),
];

/** Behavior fixture for the development API; the real backend uses the HTTP simulator. */
export function fspResults(account: string): components['schemas']['FspResult'][] {
  const id = account.trim().toUpperCase();
  if (!['FSP-DEMO-024', 'FSP-DEMO-031', 'FSP-DEMO-PENDING', 'FSP-DEMO-DQ'].includes(id)) return [];
  const pending = id === 'FSP-DEMO-PENDING';
  const disqualified = id === 'FSP-DEMO-DQ';
  const published = !pending && !disqualified;
  const result: components['schemas']['FspResult'][] = [
    {
      competitionId: 'competition-demo-api',
      competitionName: 'Учебный кубок API 2026 (вымышленное событие)',
      discipline: 'Продуктовое программирование',
      startsAt: '2026-09-20T09:00:00Z',
      endsAt: '2026-09-21T18:00:00Z',
      scope: 'team',
      teamName: 'Команда Пример',
      teamSize: 4,
      status: pending ? 'pending' : disqualified ? 'disqualified' : 'published',
      place: published ? 3 : null,
      fieldSize: 18,
      score: published ? 82 : null,
      protocolReference: pending ? null : 'DEMO-PROTOCOL-API-2026-01',
      publishedAt: pending ? null : '2026-09-22T12:00:00Z',
      synthetic: true,
    },
  ];
  if (id === 'FSP-DEMO-031') result.push(...HISTORY_031);
  if (id === 'FSP-DEMO-031')
    result.push({
      competitionId: 'competition-demo-algorithms',
      competitionName: 'Учебный алгоритмический турнир (вымышленное событие)',
      discipline: 'Алгоритмическое программирование',
      startsAt: '2026-09-24T09:00:00Z',
      endsAt: '2026-09-24T14:00:00Z',
      scope: 'individual',
      teamName: null,
      teamSize: null,
      status: 'pending',
      place: null,
      fieldSize: 48,
      score: null,
      protocolReference: null,
      publishedAt: null,
      synthetic: true,
    });
  return result;
}
