import {
  faBolt,
  faClockRotateLeft,
  faEllipsisVertical,
  faGear,
  faLayerGroup,
  faListUl,
  faRotate,
  faThumbtack,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  ActionIcon,
  Alert,
  Badge,
  Group,
  Loader,
  Menu,
  Select,
  SimpleGrid,
  Stack,
  Table,
  Text,
  Tooltip,
} from '@mantine/core';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import { ServerCan } from '@/elements/Can.tsx';
import Button from '@/elements/buttons/Button.tsx';
import ServerContentContainer from '@/elements/containers/ServerContentContainer.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import Switch from '@/elements/input/Switch.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { applyTemplate, getOverview, runNow, setBackupRoutine, setPinned, updateConfig } from './api.ts';
import RoutineListEditor, { cleanRoutines, routineColor } from './components/RoutineListEditor.tsx';
import {
  formatBytes,
  formatDate,
  type ManagedBackup,
  type Overview,
  type PlannedAction,
  type RoutineInput,
} from './lib/schemas.ts';

const REFRESH_MS = 30_000;

function toInputs(overview: Overview): RoutineInput[] {
  return overview.routines.map((r) => ({
    key: r.uuid,
    uuid: r.uuid,
    name: r.name,
    enabled: r.enabled,
    cron: r.cron,
    source: r.source,
    sourceKey: r.sourceRoutineUuid,
    keepCount: r.keepCount,
    maxAgeHours: r.maxAgeHours,
    nameTemplate: r.nameTemplate,
    ignoredFiles: r.ignoredFiles,
  }));
}

export default function ServerPage() {
  const serverUuid = useServerStore((state) => state.server.uuid);
  const { addToast } = useToast();

  const [overview, setOverview] = useState<Overview | null>(null);
  const [routines, setRoutines] = useState<RoutineInput[]>([]);
  const [enabled, setEnabled] = useState(false);
  const [deleteFailed, setDeleteFailed] = useState(true);
  const [dirty, setDirty] = useState(false);
  const [templateId, setTemplateId] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [running, setRunning] = useState<string | null>(null);

  const applyOverview = useCallback((data: Overview, resetForm: boolean) => {
    setOverview(data);
    if (resetForm) {
      setRoutines(toInputs(data));
      setEnabled(data.policy.enabled);
      setDeleteFailed(data.policy.deleteFailed);
      setTemplateId((current) => current ?? data.defaultTemplateId);
      setDirty(false);
    }
  }, []);

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

  const routineIndex = useMemo(() => {
    const map = new Map<string, { name: string; color: string }>();
    overview?.routines.forEach((r, i) => map.set(r.uuid, { name: r.name, color: routineColor(i) }));
    return map;
  }, [overview]);

  const routineName = (uuid: string | null) => (uuid ? (routineIndex.get(uuid)?.name ?? '?') : null);

  const doSave = () => {
    setSaving(true);
    updateConfig(serverUuid, { enabled, deleteFailed, routines: cleanRoutines(routines) })
      .then((data) => {
        applyOverview(data, true);
        addToast(enabled ? 'Gespeichert. Automatische Backups sind aktiv.' : 'Gespeichert (pausiert).', 'success');
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setSaving(false));
  };

  const doApplyTemplate = () => {
    if (!templateId || !overview) return;
    const template = overview.templates.find((t) => t.id === templateId);
    if (
      overview.routines.length > 0 &&
      !window.confirm(
        `Vorlage "${template?.name}" anwenden? Die bisherigen Routinen werden ersetzt. Vorhandene Backups bleiben erhalten, werden aber keiner Routine mehr zugeordnet.`,
      )
    ) {
      return;
    }

    setSaving(true);
    applyTemplate(serverUuid, templateId, true)
      .then((data) => {
        applyOverview(data, true);
        addToast(`Vorlage "${template?.name}" angewendet und aktiviert.`, 'success');
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setSaving(false));
  };

  const doRunNow = (routineUuid: string | null) => {
    setRunning(routineUuid ?? 'any');
    runNow(serverUuid, routineUuid)
      .then(() => {
        addToast('Backup gestartet.', 'success');
        load(false);
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setRunning(null));
  };

  const doPin = (backup: ManagedBackup) => {
    setPinned(serverUuid, backup.uuid, !backup.locked)
      .then(() => {
        addToast(backup.locked ? 'Backup losgelöst.' : 'Backup angepinnt – es wird nie automatisch gelöscht.', 'success');
        load(false);
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'));
  };

  const doAssign = (backup: ManagedBackup, routineUuid: string | null) => {
    setBackupRoutine(serverUuid, backup.uuid, routineUuid)
      .then(() => {
        addToast(routineUuid ? `Backup "${routineName(routineUuid)}" zugeordnet.` : 'Backup aus der Rotation genommen.', 'success');
        load(false);
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'));
  };

  const plannedBadge = (action: PlannedAction) => {
    switch (action.type) {
      case 'keep':
        return (
          <Badge color='green' variant='light'>
            Bleibt
          </Badge>
        );
      case 'move':
        return (
          <Badge color={routineIndex.get(action.to)?.color ?? 'blue'} variant='light'>
            Wird „{routineName(action.to)}“
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
      case 'unassigned':
        return (
          <Tooltip label='Gehört zu keiner aktiven Routine und wird nicht automatisch gelöscht.'>
            <Badge color='gray' variant='outline'>
              Nicht verwaltet
            </Badge>
          </Tooltip>
        );
    }
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
  const keepTotal = routines.reduce((sum, r) => sum + (Number(r.keepCount) || 0), 0);
  const limitTooLow = limits.backupLimit < keepTotal;
  const routineErrors = overview.routines.filter((r) => r.lastError);
  const selectedTemplate = overview.templates.find((t) => t.id === templateId);
  const changed = (fn: () => void) => () => {
    fn();
    setDirty(true);
  };

  return (
    <ServerContentContainer
      title='Auto-Backups'
      contentRight={
        <ServerCan action='backup-plus.manage'>
          <Button
            leftSection={<FontAwesomeIcon icon={faBolt} />}
            loading={running === 'any'}
            disabled={!overview.routines.some((r) => r.source === 'create')}
            onClick={() => doRunNow(null)}
          >
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
        {routineErrors.map((r) => (
          <Alert key={r.uuid} color='orange' title={`Routine „${r.name}“`}>
            {r.lastError}
          </Alert>
        ))}

        <SimpleGrid cols={{ base: 1, sm: 2, lg: 4 }}>
          <TitleCard title='Status' icon={faRotate}>
            <Stack gap={4}>
              <Group gap='xs'>
                <Badge color={policy.enabled ? 'green' : 'gray'}>{policy.enabled ? 'Aktiv' : 'Pausiert'}</Badge>
              </Group>
              <Text size='sm' c='dimmed'>
                Letzter Lauf: {formatDate(policy.lastRun)}
              </Text>
              <Text size='sm' c='dimmed'>
                Zeitzone: {overview.timezone}
              </Text>
            </Stack>
          </TitleCard>
          <TitleCard title='Verwaltete Backups' icon={faListUl}>
            <Text size='xl' fw={600}>
              {backups.filter((b) => b.planned.type !== 'delete').length}
            </Text>
            <Text size='sm' c='dimmed'>
              {formatBytes(backups.reduce((sum, b) => sum + b.bytes, 0))} belegt
            </Text>
          </TitleCard>
          <TitleCard title='Backup-Limit' icon={faLayerGroup}>
            <Text size='xl' fw={600} c={limitTooLow ? 'red' : undefined}>
              {limits.backupsTotal} / {limits.backupLimit}
            </Text>
            <Text size='sm' c='dimmed'>
              alle Backups dieses Servers
            </Text>
          </TitleCard>
          <TitleCard title='Routinen' icon={faClockRotateLeft}>
            <Stack gap={2}>
              {overview.routines.length === 0 && (
                <Text size='sm' c='dimmed'>
                  keine
                </Text>
              )}
              {overview.routines.map((r, i) => (
                <Group key={r.uuid} gap={6} wrap='nowrap'>
                  <Badge size='xs' color={routineColor(i)} variant='light'>
                    {r.backupCount}/{r.keepCount}
                  </Badge>
                  <Text size='sm' truncate>
                    {r.name} · nächster: {r.enabled ? (r.nextRun ? formatDate(r.nextRun) : 'gleich') : 'pausiert'}
                  </Text>
                </Group>
              ))}
            </Stack>
          </TitleCard>
        </SimpleGrid>

        <ServerCan action='backup-plus.manage'>
          <TitleCard title='Einrichtung' icon={faGear}>
            <Stack>
              <Group align='flex-end' wrap='wrap'>
                <Select
                  label='Vorlage'
                  description={selectedTemplate?.description}
                  className='flex-1'
                  style={{ minWidth: 260 }}
                  allowDeselect={false}
                  value={templateId}
                  data={overview.templates.map((t) => ({
                    value: t.id,
                    label: t.builtin ? t.name : `${t.name} (eigene)`,
                  }))}
                  onChange={setTemplateId}
                />
                <Button variant='light' loading={saving} disabled={!templateId} onClick={doApplyTemplate}>
                  Vorlage anwenden
                </Button>
              </Group>

              <Group>
                <Switch
                  label='Automatische Backups aktiv'
                  checked={enabled}
                  onChange={(e) => {
                    const value = e.currentTarget.checked;
                    changed(() => setEnabled(value))();
                  }}
                />
                <Switch
                  label='Fehlgeschlagene Backups automatisch löschen'
                  checked={deleteFailed}
                  onChange={(e) => {
                    const value = e.currentTarget.checked;
                    changed(() => setDeleteFailed(value))();
                  }}
                />
              </Group>

              {limitTooLow && (
                <Alert color='red'>
                  Das Backup-Limit dieses Servers ({limits.backupLimit}) ist kleiner als die Summe der behaltenen
                  Backups ({keepTotal}). Bitte einen Administrator, das Limit zu erhöhen (empfohlen: {keepTotal + 1}).
                </Alert>
              )}
              {!limits.canCustomize && (
                <Alert color='blue'>
                  Eigene Routinen sind vom Administrator deaktiviert – du kannst eine der Vorlagen anwenden.
                </Alert>
              )}
            </Stack>
          </TitleCard>
        </ServerCan>

        <TitleCard title='Routinen' icon={faClockRotateLeft}>
          <Stack>
            <Text size='sm' c='dimmed'>
              Jede Routine behält ihre neuesten Backups. Eine Routine vom Typ „übernehmen“ erstellt keine eigenen
              Backups: Fällt ein Backup aus ihrer Quelle heraus und sie ist laut Zeitplan dran, wird es übernommen
              statt gelöscht – so entstehen z. B. wöchentliche und monatliche Backups ohne Zusatzaufwand.
            </Text>
            <RoutineListEditor
              routines={routines}
              onChange={(next) => {
                setRoutines(next);
                setDirty(true);
              }}
              timezone={overview.timezone}
              maxRoutines={limits.maxRoutines}
              readOnly={!limits.canCustomize}
              renderExtra={(routine) => {
                const saved = overview.routines.find((r) => r.uuid === routine.uuid);
                if (!saved) return null;
                return (
                  <Group gap='md'>
                    <Text size='xs' c='dimmed'>
                      Backups: {saved.backupCount}/{saved.keepCount} · letzter Lauf: {formatDate(saved.lastRun)} ·
                      nächster: {saved.enabled ? (saved.nextRun ? formatDate(saved.nextRun) : 'gleich') : 'pausiert'}
                    </Text>
                    {saved.source === 'create' && (
                      <ServerCan action='backup-plus.manage'>
                        <Button
                          size='compact-xs'
                          variant='subtle'
                          loading={running === saved.uuid}
                          onClick={() => doRunNow(saved.uuid)}
                        >
                          Jetzt ausführen
                        </Button>
                      </ServerCan>
                    )}
                  </Group>
                );
              }}
            />
            <ServerCan action='backup-plus.manage'>
              <Group>
                <Button loading={saving} disabled={!dirty} onClick={doSave}>
                  Speichern
                </Button>
                {dirty && (
                  <Button variant='subtle' onClick={() => overview && applyOverview(overview, true)}>
                    Änderungen verwerfen
                  </Button>
                )}
              </Group>
            </ServerCan>
          </Stack>
        </TitleCard>

        <TitleCard title='Verwaltete Backups' icon={faListUl}>
          {backups.length === 0 ? (
            <Text c='dimmed' size='sm'>
              Noch keine automatischen Backups vorhanden.
            </Text>
          ) : (
            <Table.ScrollContainer minWidth={760}>
              <Table striped highlightOnHover>
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th>Name</Table.Th>
                    <Table.Th>Routine</Table.Th>
                    <Table.Th>Erstellt</Table.Th>
                    <Table.Th>Größe</Table.Th>
                    <Table.Th>Nächste Aktion</Table.Th>
                    <Table.Th />
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {backups.map((backup) => {
                    const routine = backup.routineUuid ? routineIndex.get(backup.routineUuid) : undefined;
                    return (
                      <Table.Tr key={backup.uuid}>
                        <Table.Td>
                          <Group gap={6} wrap='nowrap'>
                            {backup.locked && <FontAwesomeIcon icon={faThumbtack} />}
                            <Text size='sm'>{backup.name}</Text>
                          </Group>
                        </Table.Td>
                        <Table.Td>
                          {routine ? (
                            <Badge color={routine.color} variant='light'>
                              {routine.name}
                            </Badge>
                          ) : (
                            <Text size='sm' c='dimmed'>
                              –
                            </Text>
                          )}
                        </Table.Td>
                        <Table.Td>{formatDate(backup.created)}</Table.Td>
                        <Table.Td>{backup.completed ? formatBytes(backup.bytes) : '–'}</Table.Td>
                        <Table.Td>{plannedBadge(backup.planned)}</Table.Td>
                        <Table.Td>
                          <ServerCan action='backup-plus.manage'>
                            <Menu position='bottom-end' withinPortal>
                              <Menu.Target>
                                <ActionIcon variant='subtle' aria-label='Aktionen'>
                                  <FontAwesomeIcon icon={faEllipsisVertical} />
                                </ActionIcon>
                              </Menu.Target>
                              <Menu.Dropdown>
                                <Menu.Item
                                  leftSection={<FontAwesomeIcon icon={faThumbtack} />}
                                  onClick={() => doPin(backup)}
                                >
                                  {backup.locked ? 'Loslösen' : 'Anpinnen (nie löschen)'}
                                </Menu.Item>
                                <Menu.Divider />
                                <Menu.Label>Routine zuordnen</Menu.Label>
                                {overview.routines.map((r) => (
                                  <Menu.Item
                                    key={r.uuid}
                                    disabled={r.uuid === backup.routineUuid}
                                    onClick={() => doAssign(backup, r.uuid)}
                                  >
                                    {r.name}
                                  </Menu.Item>
                                ))}
                                <Menu.Item
                                  disabled={backup.routineUuid === null}
                                  onClick={() => doAssign(backup, null)}
                                >
                                  Keine (aus Rotation nehmen)
                                </Menu.Item>
                              </Menu.Dropdown>
                            </Menu>
                          </ServerCan>
                        </Table.Td>
                      </Table.Tr>
                    );
                  })}
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
