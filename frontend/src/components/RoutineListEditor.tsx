import { faArrowDown, faArrowUp, faPlus, faTrash } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { ActionIcon, Badge, Card, Group, Select, SimpleGrid, Stack, Text, Tooltip } from '@mantine/core';
import type { ReactNode } from 'react';
import Button from '@/elements/buttons/Button.tsx';
import CronInput from '@/elements/input/CronInput.tsx';
import NumberInput from '@/elements/input/NumberInput.tsx';
import Switch from '@/elements/input/Switch.tsx';
import TextArea from '@/elements/input/TextArea.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import { cronPresets, newKey, type RoutineInput, routineColors } from '../lib/schemas.ts';

interface Props {
  routines: RoutineInput[];
  onChange: (routines: RoutineInput[]) => void;
  timezone: string;
  maxRoutines: number;
  readOnly?: boolean;
  /** Zusätzliche Inhalte pro Routine (Status, "Jetzt ausführen" …). */
  renderExtra?: (routine: RoutineInput) => ReactNode;
}

export function emptyRoutine(position: number): RoutineInput {
  return {
    key: newKey(),
    uuid: null,
    name: position === 0 ? 'Stündlich' : `Routine ${position + 1}`,
    enabled: true,
    cron: '0 0 * * * *',
    source: 'create',
    sourceKey: null,
    keepCount: 3,
    maxAgeHours: 0,
    nameTemplate: 'Auto {routine} {date}',
    ignoredFiles: [],
  };
}

export function routineColor(index: number): string {
  return routineColors[index % routineColors.length];
}

export default function RoutineListEditor({ routines, onChange, timezone, maxRoutines, readOnly, renderExtra }: Props) {
  const update = (index: number, patch: Partial<RoutineInput>) => {
    onChange(routines.map((r, i) => (i === index ? { ...r, ...patch } : r)));
  };

  const remove = (index: number) => {
    const removed = routines[index];
    onChange(
      routines
        .filter((_, i) => i !== index)
        .map((r) => (r.sourceKey === removed.key ? { ...r, sourceKey: null } : r)),
    );
  };

  const move = (index: number, delta: number) => {
    const target = index + delta;
    if (target < 0 || target >= routines.length) return;
    const next = [...routines];
    [next[index], next[target]] = [next[target], next[index]];
    onChange(next);
  };

  const add = () => {
    const routine = emptyRoutine(routines.length);
    const firstCreate = routines.find((r) => r.source === 'create');
    if (firstCreate) {
      // Eine weitere Routine soll typischerweise Backups übernehmen (z. B. "Wöchentlich").
      routine.source = 'promote';
      routine.sourceKey = firstCreate.key;
      routine.cron = '0 0 0 * * 1';
      routine.keepCount = 1;
      routine.name = 'Wöchentlich';
      if (routines.some((r) => r.name === routine.name)) routine.name = `Routine ${routines.length + 1}`;
    }
    onChange([...routines, routine]);
  };

  const keepTotal = routines.reduce((sum, r) => sum + (Number(r.keepCount) || 0), 0);

  return (
    <Stack>
      {routines.length === 0 && (
        <Text c='dimmed' size='sm'>
          Noch keine Routine angelegt. Wende oben eine Vorlage an oder füge eine Routine hinzu.
        </Text>
      )}

      {routines.map((routine, index) => {
        const sourceOptions = routines
          .filter((r) => r.key !== routine.key)
          .map((r) => ({ value: r.key, label: r.name || '(ohne Namen)' }));

        return (
          <Card key={routine.key} withBorder radius='md' p='md'>
            <Stack gap='sm'>
              <Group justify='space-between' wrap='nowrap'>
                <Group gap='xs' wrap='nowrap'>
                  <Badge color={routineColor(index)} variant='light'>
                    {index + 1}
                  </Badge>
                  <Text fw={600}>{routine.name || '(ohne Namen)'}</Text>
                  {!routine.enabled && (
                    <Badge color='gray' variant='outline'>
                      pausiert
                    </Badge>
                  )}
                </Group>
                {!readOnly && (
                  <Group gap={4} wrap='nowrap'>
                    <Tooltip label='Nach oben'>
                      <ActionIcon variant='subtle' disabled={index === 0} onClick={() => move(index, -1)}>
                        <FontAwesomeIcon icon={faArrowUp} />
                      </ActionIcon>
                    </Tooltip>
                    <Tooltip label='Nach unten'>
                      <ActionIcon
                        variant='subtle'
                        disabled={index === routines.length - 1}
                        onClick={() => move(index, 1)}
                      >
                        <FontAwesomeIcon icon={faArrowDown} />
                      </ActionIcon>
                    </Tooltip>
                    <Tooltip label='Routine entfernen (ihre Backups bleiben erhalten)'>
                      <ActionIcon variant='subtle' color='red' onClick={() => remove(index)}>
                        <FontAwesomeIcon icon={faTrash} />
                      </ActionIcon>
                    </Tooltip>
                  </Group>
                )}
              </Group>

              {renderExtra?.(routine)}

              <SimpleGrid cols={{ base: 1, sm: 2 }}>
                <TextInput
                  label='Name'
                  value={routine.name}
                  disabled={readOnly}
                  maxLength={64}
                  onChange={(e) => update(index, { name: e.currentTarget.value })}
                />
                <Select
                  label='Art'
                  disabled={readOnly}
                  allowDeselect={false}
                  value={routine.source}
                  data={[
                    { value: 'create', label: 'Neues Backup erstellen' },
                    { value: 'promote', label: 'Backup aus anderer Routine übernehmen' },
                  ]}
                  onChange={(value) =>
                    update(index, {
                      source: value === 'promote' ? 'promote' : 'create',
                      sourceKey: value === 'promote' ? (sourceOptions[0]?.value ?? null) : null,
                    })
                  }
                />
              </SimpleGrid>

              {routine.source === 'promote' && (
                <Select
                  label='Quelle'
                  description='Fällt ein Backup aus dieser Routine heraus und diese Routine ist laut Zeitplan dran, wird es übernommen statt gelöscht.'
                  disabled={readOnly}
                  value={routine.sourceKey}
                  data={sourceOptions}
                  placeholder='Routine wählen'
                  onChange={(value) => update(index, { sourceKey: value })}
                />
              )}

              <SimpleGrid cols={{ base: 1, sm: 2 }}>
                <Stack gap={4}>
                  <CronInput
                    label={routine.source === 'create' ? 'Zeitplan (wann gesichert wird)' : 'Zeitplan (wann übernommen wird)'}
                    description={`Cron mit Sekunden, Zeitzone ${timezone}`}
                    value={routine.cron}
                    timezone={timezone}
                    onChange={(value) => !readOnly && update(index, { cron: value })}
                  />
                </Stack>
                <Select
                  label='Schnellauswahl'
                  placeholder='Vorgabe wählen …'
                  disabled={readOnly}
                  value={cronPresets.some((p) => p.value === routine.cron) ? routine.cron : null}
                  data={cronPresets}
                  onChange={(value) => value && update(index, { cron: value })}
                />
              </SimpleGrid>

              <SimpleGrid cols={{ base: 1, sm: 3 }}>
                <NumberInput
                  label='Behalten'
                  description='Anzahl der neuesten Backups'
                  min={1}
                  max={500}
                  disabled={readOnly}
                  value={routine.keepCount}
                  onChange={(value) => update(index, { keepCount: Number(value) || 1 })}
                />
                <NumberInput
                  label='Höchstalter (Stunden)'
                  description='0 = unbegrenzt'
                  min={0}
                  max={87600}
                  disabled={readOnly}
                  value={routine.maxAgeHours}
                  onChange={(value) => update(index, { maxAgeHours: Number(value) || 0 })}
                />
                <Switch
                  label='Aktiv'
                  mt='lg'
                  disabled={readOnly}
                  checked={routine.enabled}
                  onChange={(e) => update(index, { enabled: e.currentTarget.checked })}
                />
              </SimpleGrid>

              {routine.source === 'create' && (
                <SimpleGrid cols={{ base: 1, sm: 2 }}>
                  <TextInput
                    label='Namensvorlage'
                    description='{routine}, {date}, {day}, {time}, {server}'
                    disabled={readOnly}
                    value={routine.nameTemplate}
                    maxLength={200}
                    onChange={(e) => update(index, { nameTemplate: e.currentTarget.value })}
                  />
                  <TextArea
                    label='Ignorierte Dateien'
                    description='Eine Regel pro Zeile, z. B. logs/ oder *.tmp'
                    autosize
                    minRows={1}
                    disabled={readOnly}
                    value={routine.ignoredFiles.join('\n')}
                    onChange={(e) => update(index, { ignoredFiles: e.currentTarget.value.split('\n') })}
                  />
                </SimpleGrid>
              )}
            </Stack>
          </Card>
        );
      })}

      {!readOnly && (
        <Group justify='space-between'>
          <Button
            variant='light'
            leftSection={<FontAwesomeIcon icon={faPlus} />}
            disabled={routines.length >= maxRoutines}
            onClick={add}
          >
            Routine hinzufügen
          </Button>
          <Text size='sm' c='dimmed'>
            Insgesamt {keepTotal} Backups
          </Text>
        </Group>
      )}
    </Stack>
  );
}

/** Ignorierte Dateien vor dem Speichern säubern. */
export function cleanRoutines(routines: RoutineInput[]): RoutineInput[] {
  return routines.map((r) => ({
    ...r,
    name: r.name.trim(),
    cron: r.cron.trim(),
    ignoredFiles: r.ignoredFiles.map((f) => f.trim()).filter((f) => f.length > 0),
  }));
}
