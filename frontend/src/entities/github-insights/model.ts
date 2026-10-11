import type { components } from '@/shared/api';
import { pluralRu } from '@/shared/lib';
import type { Tone } from '@/shared/ui';

type S = components['schemas'];

export type GitHubReport = S['GitHubReport'];
export type GitHubSignal = S['GitHubSignal'];
export type GitHubSource = S['GitHubSource'];
export type GitHubIndex = S['GitHubIndex'];
export type GitHubFlag = S['GitHubFlag'];
export type GitHubDimension = S['GitHubDimension'];
export type HygieneSignal = GitHubReport['repositories'][number]['hygiene'][number];
export type StackCategory = S['GitHubStackItem']['category'];
export type GitHubAnalysis = NonNullable<GitHubSource['analysis']>;

export const bandLabel: Record<GitHubIndex['band'], string> = {
  weak: 'Слабый сигнал',
  moderate: 'Умеренный сигнал',
  strong: 'Сильный сигнал',
  very_strong: 'Очень сильный сигнал',
};

export const bandTone: Record<GitHubIndex['band'], Tone> = {
  weak: 'neutral',
  moderate: 'info',
  strong: 'success',
  very_strong: 'success',
};

export const confidenceLabel: Record<GitHubIndex['confidence'], string> = {
  low: 'низкая',
  medium: 'средняя',
  high: 'высокая',
};

export const hygieneLabel: Record<HygieneSignal, string> = {
  readme: 'README',
  license: 'Лицензия',
  ci: 'CI',
  tests: 'Тесты',
  lockfile: 'Lock-файл',
  lint: 'Линтеры',
  dependency_updates: 'Обновление зависимостей',
  container: 'Контейнер',
  description: 'Описание',
};

/** Display order and Russian names of stack categories. */
export const stackCategoryLabel: Record<StackCategory, string> = {
  language: 'Языки',
  backend: 'Бэкенд',
  frontend: 'Фронтенд',
  data: 'Данные и хранилища',
  ai: 'AI',
  infrastructure: 'Инфраструктура и DevOps',
  quality: 'Качество',
  mobile: 'Мобильная разработка',
  tooling: 'Инструменты',
};

/** Groups items by category in the order of `stackCategoryLabel`, skipping empty categories. */
export function byStackCategory<T extends { category: StackCategory }>(items: T[]) {
  return (Object.keys(stackCategoryLabel) as StackCategory[])
    .map((category) => ({ category, items: items.filter((item) => item.category === category) }))
    .filter((group) => group.items.length > 0);
}

export const weekdayShortLabel = ['пн', 'вт', 'ср', 'чт', 'пт', 'сб', 'вс'] as const;

/** `105` → «1 ч 45 мин». */
export function formatMinutes(minutes: number) {
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (!hours) return `${rest} мин`;
  return rest ? `${hours} ч ${rest} мин` : `${hours} ч`;
}

/** `300` → «UTC+5», `-210` → «UTC−3:30». */
export function formatOffset(minutes: number) {
  const sign = minutes < 0 ? '−' : '+';
  const absolute = Math.abs(minutes);
  const hours = Math.floor(absolute / 60);
  const rest = absolute % 60;
  return `UTC${sign}${hours}${rest ? `:${String(rest).padStart(2, '0')}` : ''}`;
}

export const yearsLabel = (count: number) => `${count} ${pluralRu(count, 'год', 'года', 'лет')}`;
