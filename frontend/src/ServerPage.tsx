import {
  faBolt,
  faCalendarDays,
  faClockRotateLeft,
  faEllipsisVertical,
  faListUl,
  faRotate,
  faThumbtack,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { ActionIcon, Alert, Badge, Group, Loader, Menu, SimpleGrid, Stack, Table, Text, Tooltip } from '@mantine/core';
import { useForm } from '@mantine/form';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import { ServerCan } from '@/elements/Can.tsx';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import NumberInput from '@/elements/input/NumberInput.tsx';
import Switch from '@/elements/input/Switch.tsx';
import TextArea from '@/elements/input/TextArea.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { getOverview, runNow, setPinned, setTier, updatePolicy } from './api.ts';
import {
  formatBytes,
  formatDate,
  type ManagedBackup,
  type Overview,
  type PlannedAction,
  type PolicyUpdate,
  policyUpdateSchema,
  type Tier,
  tierColors,
  tierLabels,
} from './lib/schemas.ts';

const REFRESH_MS = 30_000;

function PlannedActionBadge({ action }: { action: PlannedAction }) {
  switch (action.type) {
    case 'keep':
      return (
        <Badge color='green' variant='light'>
          Bleibt
        </Badge>
      );
    case 'promote':
      return (
        <Badge color={tierColors[action.to]} variant='light'>
          Wird {tierLabels[action.to].toLowerCase()}
        </Badge>
      );
    case 'delete':
      return (
        <Tooltip label={action.reason}>
          <Badge color='red' variant='light'>
            Wird gelöscht
          </Badge>
        </Tooltip>
      );
    case 'pinned':
      return (
        <Badge color='yellow' variant='light' leftSection={<FontAwesomeIcon icon={faThumbtack} />}>
          Angepinnt
        </Badge>
      );
    case 'running':
      return (
        <Badge color='gray' variant='light' leftSection={<Loader size={10} />}>
          Läuft
        </Badge>
      );
    case 'failed_kept':
      return (
        <Badge color='red' variant='outline'>
          Fehlgeschlagen
        </Badge>
      );
  }
}

function toForm(overview: Overview): PolicyUpdate {
  const p = overview.policy;
  return {
    enabled: p.enabled,
    intervalHours: p.intervalHours,
    minuteOffset: p.minuteOffset,
    keepHourly: p.keepHourly,
    keepDaily: p.keepDaily,
    keepWeekly: p.keepWeekly,
    keepMonthly: p.keepMonthly,
    nameTemplate: p.nameTemplate,
    ignoredFiles: p.ignoredFiles,
    deleteFailed: p.deleteFailed,
  };
}

export default function ServerPage() {
  const serverUuid = useServerStore((state) => state.server.uuid);
  const { addToast } = useToast();

  const [overview, setOverview] = useState<Overview | null>(null);
  const [saving, setSaving] = useState(false);
  const [running, setRunning] = useState(false);
  const [ignoredText, setIgnoredText] = useState('');

  const form = useForm<PolicyUpdate>({
    initialValues: {
      enabled: false,
      intervalHours: 1,
      minuteOffset: 0,
      keepHourly: 3,
      keepDaily: 0,
      keepWeekly: 1,
      keepMonthly: 1,
      nameTemplate: 'Auto {tier} {date}',
      ignoredFiles: [],
      deleteFailed: true,
    },
    validateInputOnBlur: true,
    validate: zod4Resolver(policyUpdateSchema),
  });

  const applyOverview = useCallback(
    (data: Overview, resetForm: boolean) => {
      setOverview(data);
      if (resetForm) {
        const values = toForm(data);
        form.setValues(values);
        form.resetDirty(values);
        setIgnoredText(values.ignoredFiles.join('\n'));
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );

  const load = useCallback(
    (resetForm: boolean) => {
      getOverview(serverUuid)
        .then((data) => applyOverview(data, resetForm))
        .catch((err) => addToast(httpErrorToHuman(err), 'error'));
    },
    [serverUuid, applyOverview, addToast],
  );

  useEffect(() => {
    load(true);
    const timer = setInterval(() => load(false), REFRESH_MS);
    return () => clearInterval(timer);
  }, [load]);

  const keepTotal = form.values.keepHourly + form.values.keepDaily + form.values.keepWeekly + form.values.keepMonthly;

  const summary = useMemo(() => {
    const parts: string[] = [];
    const v = form.values;
    if (v.keepHourly > 0) parts.push(`${v.keepHourly}× stündlich (die letzten ${v.keepHourly * v.intervalHours} Std.)`);
    if (v.keepDaily > 0) parts.push(`${v.keepDaily}× täglich`);
    if (v.keepWeekly > 0) parts.push(`${v.keepWeekly}× wöchentlich`);
    if (v.keepMonthly > 0) parts.push(`${v.keepMonthly}× monatlich`);
    return parts.join(', ');
  }, [form.values]);

  const doSave = (values: PolicyUpdate) => {
    setSaving(true);
    const payload: PolicyUpdate = {
      ...values,
      ignoredFiles: ignoredText
        .split('\n')
        .map((l) => l.trim())
        .filter((l) => l.length > 0),
    };

    updatePolicy(serverUuid, payload)
      .then((data) => {
        applyOverview(data, true);
        addToast(
          payload.enabled
            ? 'Richtlinie gespeichert. Automatische Backups sind aktiv.'
            : 'Richtlinie gespeichert (pausiert).',
          'success',
        );
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setSaving(false));
  };

  const doRunNow = () => {
    setRunning(true);
    runNow(serverUuid)
      .then(() => {
        addToast('Backup gestartet. Alte Backups werden gemäß Richtlinie rotiert.', 'success');
        load(false);
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setRunning(false));
  };

  const doPin = (backup: ManagedBackup) => {
    setPinned(serverUuid, backup.uuid, !backup.locked)
      .then(() => {
        addToast(backup.locked ? 'Backup losgelöst.' : 'Backup angepinnt – es wird nie automatisch gelöscht.', 'success');
        load(false);
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'));
  };

  const doSetTier = (backup: ManagedBackup, tier: Tier) => {
    setTier(serverUuid, backup.uuid, tier)
      .then(() => {
        addToast(`Backup als ${tierLabels[tier].toLowerCase()} markiert.`, 'success');
        load(false);
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'));
  };

  if (!overview) {
    return (
      <ServerContentContainer title='Auto-Backups'>
        <Group justify='center' p='xl'>
          <Loader />
        </Group>
      </ServerContentContainer>
    );
  }

  const { policy, limits, backups, log } = overview;
  const limitTooLow = limits.backupLimit < keepTotal;
  const keptCount = backups.filter((b) => b.planned.type !== 'delete').length;

  return (
    <ServerContentContainer
      title='Auto-Backups'
      contentRight={
        <ServerCan action='backup-plus.manage'>
          <Button leftSection={<FontAwesomeIcon icon={faBolt} />} loading={running} onClick={doRunNow}>
            Jetzt sichern
          </Button>
        </ServerCan>
      }
    >
      <Stack>
        {!limits.globallyEnabled && (
          <Alert color='yellow' title='Global deaktiviert'>
            Ein Administrator hat Backup Plus pausiert. Es werden derzeit keine automatischen Backups erstellt oder
            gelöscht.
          </Alert>
        )}
        {policy.lastError && (
          <Alert color='red' title='Letzter Lauf fehlgeschlagen'>
            {policy.lastError}
          </Alert>
        )}

        <SimpleGrid cols={{ base: 1, sm: 2, lg: 4 }}>
          <TitleCard title='Status' icon={faRotate}>
            <Stack gap={4}>
              <Group gap='xs'>
                <Badge color={policy.enabled ? 'green' : 'gray'}>{policy.enabled ? 'Aktiv' : 'Pausiert'}</Badge>
              </Group>
              <Text size='sm' c='dimmed'>
                Nächster Lauf: {policy.enabled ? (policy.nextRun ? formatDate(policy.nextRun) : 'in Kürze') : '–'}
              </Text>
              <Text size='sm' c='dimmed'>
                Letzter Lauf: {formatDate(policy.lastRun)}
              </Text>
            </Stack>
          </TitleCard>
          <TitleCard title='Verwaltete Backups' icon={faListUl}>
            <Text size='xl' fw={600}>
              {keptCount} / {keepTotal}
            </Text>
            <Text size='sm' c='dimmed'>
              {formatBytes(backups.reduce((sum, b) => sum + b.bytes, 0))} belegt
            </Text>
          </TitleCard>
          <TitleCard title='Backup-Limit' icon={faCalendarDays}>
            <Text size='xl' fw={600} c={limitTooLow ? 'red' : undefined}>
              {limits.backupsTotal} / {limits.backupLimit}
            </Text>
            <Text size='sm' c='dimmed'>
              alle Backups dieses Servers
            </Text>
          </TitleCard>
          <TitleCard title='Rotation' icon={faClockRotateLeft}>
            <Text size='sm'>{summary || 'Keine Stufe aktiv'}</Text>
          </TitleCard>
        </SimpleGrid>

        <ServerCan action='backup-plus.manage'>
          <TitleCard title='Richtlinie' icon={faCalendarDays}>
            <form onSubmit={form.onSubmit(doSave)}>
              <Stack>
                <Switch
                  label='Automatische Backups aktivieren'
                  description='Beim Aktivieren wird innerhalb einer Minute das erste Backup erstellt.'
                  {...form.getInputProps('enabled', { type: 'checkbox' })}
                />

                <SimpleGrid cols={{ base: 1, sm: 2 }}>
                  <NumberInput
                    label='Intervall (Stunden)'
                    description={`Wie oft ein neues Backup erstellt wird (min. ${limits.minIntervalHours}).`}
                    min={limits.minIntervalHours}
                    max={168}
                    {...form.getInputProps('intervalHours')}
                  />
                  <NumberInput
                    label='Minute'
                    description='Zu welcher Minute der Stunde gesichert wird (0–59, UTC-Stunde).'
                    min={0}
                    max={59}
                    {...form.getInputProps('minuteOffset')}
                  />
                </SimpleGrid>

                <SimpleGrid cols={{ base: 2, sm: 4 }}>
                  <NumberInput label='Stündlich behalten' min={1} max={100} {...form.getInputProps('keepHourly')} />
                  <NumberInput label='Täglich behalten' min={0} max={100} {...form.getInputProps('keepDaily')} />
                  <NumberInput label='Wöchentlich behalten' min={0} max={100} {...form.getInputProps('keepWeekly')} />
                  <NumberInput label='Monatlich behalten' min={0} max={100} {...form.getInputProps('keepMonthly')} />
                </SimpleGrid>

                <Text size='sm' c='dimmed'>
                  Ergebnis: insgesamt {keepTotal} Backups ({summary || '–'}). Fällt ein stündliches Backup aus der
                  Rotation, wird es zum monatlichen, wöchentlichen oder täglichen Backup befördert, sobald diese Stufe
                  fällig ist – sonst gelöscht. Alles Ältere wird automatisch entfernt.
                </Text>

                {limitTooLow && (
                  <Alert color='red'>
                    Das Backup-Limit dieses Servers ({limits.backupLimit}) ist kleiner als die gewünschte Anzahl (
                    {keepTotal}). Bitte einen Administrator, das Limit zu erhöhen (empfohlen: {keepTotal + 1}).
                  </Alert>
                )}
                {keepTotal > limits.maxKeepTotal && (
                  <Alert color='red'>Der Administrator erlaubt höchstens {limits.maxKeepTotal} Backups pro Server.</Alert>
                )}

                <TextInput
                  label='Namensvorlage'
                  description='Platzhalter: {date}, {day}, {time}, {tier}, {server}'
                  {...form.getInputProps('nameTemplate')}
                />

                <TextArea
                  label='Ignorierte Dateien'
                  description='Eine Regel pro Zeile (wie .gitignore), z. B. logs/ oder *.tmp'
                  autosize
                  minRows={2}
                  value={ignoredText}
                  onChange={(e) => setIgnoredText(e.currentTarget.value)}
                />

                <Switch
                  label='Fehlgeschlagene Backups automatisch löschen'
                  {...form.getInputProps('deleteFailed', { type: 'checkbox' })}
                />

                <Group>
                  <Button type='submit' loading={saving} disabled={!form.isValid()}>
                    Speichern
                  </Button>
                </Group>
              </Stack>
            </form>
          </TitleCard>
        </ServerCan>

        <TitleCard title='Verwaltete Backups' icon={faListUl}>
          {backups.length === 0 ? (
            <Text c='dimmed' size='sm'>
              Noch keine automatischen Backups vorhanden.
            </Text>
          ) : (
            <Table.ScrollContainer minWidth={700}>
              <Table striped highlightOnHover>
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th>Name</Table.Th>
                    <Table.Th>Stufe</Table.Th>
                    <Table.Th>Erstellt</Table.Th>
                    <Table.Th>Größe</Table.Th>
                    <Table.Th>Nächste Aktion</Table.Th>
                    <Table.Th />
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {backups.map((backup) => (
                    <Table.Tr key={backup.uuid}>
                      <Table.Td>
                        <Group gap={6} wrap='nowrap'>
                          {backup.locked && <FontAwesomeIcon icon={faThumbtack} />}
                          <Text size='sm'>{backup.name}</Text>
                        </Group>
                      </Table.Td>
                      <Table.Td>
                        <Badge color={tierColors[backup.tier]} variant='light'>
                          {tierLabels[backup.tier]}
                        </Badge>
                      </Table.Td>
                      <Table.Td>{formatDate(backup.created)}</Table.Td>
                      <Table.Td>{backup.completed ? formatBytes(backup.bytes) : '–'}</Table.Td>
                      <Table.Td>
                        <PlannedActionBadge action={backup.planned} />
                      </Table.Td>
                      <Table.Td>
                        <ServerCan action='backup-plus.manage'>
                          <Menu position='bottom-end' withinPortal>
                            <Menu.Target>
                              <ActionIcon variant='subtle' aria-label='Aktionen'>
                                <FontAwesomeIcon icon={faEllipsisVertical} />
                              </ActionIcon>
                            </Menu.Target>
                            <Menu.Dropdown>
                              <Menu.Item leftSection={<FontAwesomeIcon icon={faThumbtack} />} onClick={() => doPin(backup)}>
                                {backup.locked ? 'Loslösen' : 'Anpinnen (nie löschen)'}
                              </Menu.Item>
                              <Menu.Divider />
                              <Menu.Label>Stufe ändern</Menu.Label>
                              {(Object.keys(tierLabels) as Tier[]).map((tier) => (
                                <Menu.Item
                                  key={tier}
                                  disabled={tier === backup.tier}
                                  onClick={() => doSetTier(backup, tier)}
                                >
                                  {tierLabels[tier]}
                                </Menu.Item>
                              ))}
                            </Menu.Dropdown>
                          </Menu>
                        </ServerCan>
                      </Table.Td>
                    </Table.Tr>
                  ))}
                </Table.Tbody>
              </Table>
            </Table.ScrollContainer>
          )}
        </TitleCard>

        <TitleCard title='Protokoll' icon={faClockRotateLeft}>
          {log.length === 0 ? (
            <Text c='dimmed' size='sm'>
              Noch keine Einträge.
            </Text>
          ) : (
            <Stack gap={4}>
              {log.map((entry, i) => (
                <Group key={i} gap='sm' wrap='nowrap' align='flex-start'>
                  <Text size='xs' c='dimmed' style={{ whiteSpace: 'nowrap' }}>
                    {formatDate(entry.created)}
                  </Text>
                  <Badge
                    size='xs'
                    variant='light'
                    color={entry.level === 'error' ? 'red' : entry.level === 'warning' ? 'yellow' : 'blue'}
                  >
                    {entry.level}
                  </Badge>
                  <Text size='sm'>{entry.message}</Text>
                </Group>
              ))}
            </Stack>
          )}
        </TitleCard>
      </Stack>
    </ServerContentContainer>
  );
}
