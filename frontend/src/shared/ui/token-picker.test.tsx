import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { describe, expect, it } from 'vitest';
import { TokenPicker } from './field';

const options = [
  { value: 'onec_language', label: 'Встроенный язык 1С', group: '1С' },
  { value: 'onec_queries', label: 'Язык запросов 1С', group: '1С' },
  { value: 'python', label: 'Python', group: 'Языки программирования' },
  { value: 'rust', label: 'Rust', group: 'Языки программирования' },
];

function Harness({ max = 2 }: { max?: number }) {
  const [selected, setSelected] = useState<string[]>([]);
  return <TokenPicker id="stack" label="Стек" options={options} selected={selected} onChange={setSelected} max={max} />;
}

describe('TokenPicker', () => {
  it('filters a long catalog, adds from the keyboard and removes chips', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const input = screen.getByRole('combobox', { name: 'Стек' });
    await user.type(input, '1с');
    const listbox = screen.getByRole('listbox', { name: 'Стек' });
    expect(listbox.textContent).toContain('Встроенный язык 1С');
    expect(listbox.textContent).not.toContain('Python');
    await user.keyboard('{ArrowDown}{Enter}');
    expect(screen.getByRole('button', { name: 'Удалить «Язык запросов 1С»' })).toBeTruthy();
    await user.type(input, 'pyt{Enter}');
    expect(screen.getByRole('button', { name: 'Удалить «Python»' })).toBeTruthy();
    // The limit disables further input.
    expect((input as HTMLInputElement).disabled).toBe(true);
    await user.click(screen.getByRole('button', { name: 'Удалить «Python»' }));
    expect((input as HTMLInputElement).disabled).toBe(false);
  });
});
