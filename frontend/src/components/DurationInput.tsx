import { Group, Select } from '@mantine/core';
import { useEffect, useState } from 'react';
import NumberInput from '@/elements/input/NumberInput.tsx';

const UNITS = [
  { value: '86400', label: 'd' },
  { value: '3600', label: 'h' },
  { value: '60', label: 'min' },
  { value: '1', label: 's' },
];

/** Largest unit that divides the value evenly. */
function unitFor(seconds: number): number {
  return Number(UNITS.find((u) => seconds % Number(u.value) === 0)?.value ?? 1);
}

interface Props {
  label: string;
  description?: string;
  /** Seconds. */
  value: number;
  onChange: (seconds: number) => void;
  min: number;
  max: number;
  disabled?: boolean;
  error?: string;
}

/** Number + unit (s / min / h / d), stored as seconds. */
export default function DurationInput({ label, description, value, onChange, min, max, disabled, error }: Props) {
  const [unit, setUnit] = useState(() => unitFor(value));

  // Follow external changes (e.g. after loading) without fighting the user's unit choice.
  useEffect(() => {
    if (value % unit !== 0) setUnit(unitFor(value));
  }, [value, unit]);

  return (
    <Group gap='xs' align='flex-end' wrap='nowrap'>
      <NumberInput
        className='flex-1'
        label={label}
        description={description}
        error={error}
        disabled={disabled}
        min={1}
        allowDecimal={false}
        value={value / unit}
        onChange={(v) => onChange(Math.min(max, Math.max(1, Number(v) || 1) * unit))}
      />
      <Select
        w={90}
        disabled={disabled}
        allowDeselect={false}
        data={UNITS}
        value={String(unit)}
        onChange={(v) => {
          const next = Number(v ?? 1);
          setUnit(next);
          onChange(Math.min(max, Math.max(min, Math.max(1, Math.round(value / next)) * next)));
        }}
      />
    </Group>
  );
}
