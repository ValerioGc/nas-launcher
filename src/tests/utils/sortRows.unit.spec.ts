import { describe, expect, it } from 'vitest';
import { sortRows } from '@/utils/sortRows';

interface Row {
  id: number;
  name: string;
  size: number;
  system: boolean;
}

const rows: Row[] = [
  { id: 10, name: 'Task 10', size: 900, system: true },
  { id: 2, name: 'Task 2', size: 20, system: false },
];

describe('row ordering', () => {
  it.each(['name', 'id', 'size', 'system'] as (keyof Row)[])(
    'sorts %s both ways without touching the original rows',
    (column) => {
      expect(sortRows(rows, column, true, 'it', 'id').map((row) => row.id)).toEqual([2, 10]);
      expect(sortRows(rows, column, false, 'it', 'id').map((row) => row.id)).toEqual([10, 2]);
      expect(rows[0]?.id).toBe(10);
    },
  );

  it('breaks a tie with the field it was given', () => {
    const tied: Row[] = [
      { id: 3, name: 'Uguale', size: 1, system: false },
      { id: 1, name: 'Uguale', size: 1, system: false },
      { id: 2, name: 'Uguale', size: 1, system: false },
    ];

    expect(sortRows(tied, 'name', true, 'it', 'id').map((row) => row.id)).toEqual([1, 2, 3]);
    expect(sortRows(tied, 'name', false, 'it', 'id').map((row) => row.id)).toEqual([1, 2, 3]);
  });

  it('compares text the way the reader would', () => {
    const files: Row[] = [
      { id: 1, name: 'file10', size: 0, system: false },
      { id: 2, name: 'file9', size: 0, system: false },
      { id: 3, name: 'Àlbum', size: 0, system: false },
    ];

    expect(sortRows(files, 'name', true, 'it', 'id').map((row) => row.name)).toEqual([
      'Àlbum',
      'file9',
      'file10',
    ]);
  });
});
