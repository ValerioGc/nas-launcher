export function sortRows<T extends object>(
  rows: readonly T[],
  column: keyof T,
  ascending: boolean,
  locale: string,
  tiebreak: keyof T,
): T[] {
  const collator = new Intl.Collator(locale, { numeric: true, sensitivity: 'base' });

  const compare = (left: T[keyof T], right: T[keyof T]) => {
    if (typeof left === 'number' && typeof right === 'number') {
      return left - right;
    }

    if (typeof left === 'boolean' && typeof right === 'boolean') {
      return Number(left) - Number(right);
    }

    return collator.compare(String(left), String(right));
  };

  return [...rows].sort((left, right) => {
    const order = compare(left[column], right[column]);
    return (ascending ? order : -order) || compare(left[tiebreak], right[tiebreak]);
  });
}
