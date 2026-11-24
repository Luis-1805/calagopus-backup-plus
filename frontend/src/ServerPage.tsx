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
  type DeleteReason,
  formatBytes,
  formatDate,
  type ManagedBackup,
  type Overview,
  type PlannedAction,
  type RoutineInput,
} from './lib/schemas.ts';
import { useExtTranslations } from './translations.ts';

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
  const { t: tExt } = useExtTranslations();

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

  const routineName = (uuid: string | null) => (uuid ? (routineIndex.get(uuid)?.name ?? '?') : '');

  const nextRunLabel = (enabledRoutine: boolean, nextRun: Date | null) =>
    !enabledRoutine ? tExt('common.paused', {}) : nextRun ? formatDate(nextRun) : tExt('common.soon', {});

  const reasonText = (reason: DeleteReason) => {
    switch (reason.kind) {
      case 'failed':
        return tExt('reason.failed', {});
      case 'too_old':
        return tExt('reason.tooOld', { routine: reason.routine });
      case 'over_keep':
        return tExt('reason.overKeep', { routine: reason.routine, keep: reason.keep });
    }
  };

  const doSave = () => {
    setSaving(true);
    updateConfig(serverUuid, { enabled, deleteFailed, routines: cleanRoutines(routines) })
      .then((data) => {
        applyOverview(data, true);
        addToast(enabled ? tExt('server.toast.saved', {}) : tExt('server.toast.savedPaused', {}), 'success');
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setSaving(false));
  };

  const doApplyTemplate = () => {
    if (!templateId || !overview) return;
    const template = overview.templates.find((t) => t.id === templateId);
    if (
      overview.routines.length > 0 &&
      !window.confirm(tExt('server.setup.confirmTemplate', { name: template?.name ?? templateId }))
    ) {
      return;
    }

    setSaving(true);
    applyTemplate(serverUuid, templateId, true)
      .then((data) => {
        applyOverview(data, true);
        addToast(tExt('server.toast.templateApplied', { name: template?.name ?? templateId }), 'success');
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setSaving(false));
  };

  const doRunNow = (routineUuid: string | null) => {
    setRunning(routineUuid ?? 'any');
    runNow(serverUuid, routineUuid)
      .then(() => {
        addToast(tExt('server.toast.backupStarted', {}), 'success');
        load(false);
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setRunning(null));
  };

  const doPin = (backup: ManagedBackup) => {
    setPinned(serverUuid, backup.uuid, !backup.locked)
      .then(() => {
        addToast(backup.locked ? tExt('server.toast.unpinned', {}) : tExt('server.toast.pinned', {}), 'success');
        load(false);
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'));
  };

  const doAssign = (backup: ManagedBackup, routineUuid: string | null) => {
    setBackupRoutine(serverUuid, backup.uuid, routineUuid)
      .then(() => {
        addToast(
          routineUuid
            ? tExt('server.toast.assigned', { routine: routineName(routineUuid) })
            : tExt('server.toast.unassigned', {}),
          'success',
        );
        load(false);
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'));
  };

  const plannedBadge = (action: PlannedAction) => {
    switch (action.type) {
      case 'keep':
        return (
          <Badge color='green' variant='light'>
            {tExt('server.planned.keep', {})}
          </Badge>
        );
      case 'move':
        return (
          <Badge color={routineIndex.get(action.to)?.color ?? 'blue'} variant='light'>
            {tExt('server.planned.move', { routine: routineName(action.to) })}
          </Badge>
        );
      case 'delete':
        return (
          <Tooltip label={reasonText(action.reason)}>
            <Badge color='red' variant='light'>
              {tExt('server.planned.delete', {})}
            </Badge>
          </Tooltip>
        );
      case 'pinned':
        return (
          <Badge color='yellow' variant='light' leftSection={<FontAwesomeIcon icon={faThumbtack} />}>
            {tExt('server.planned.pinned', {})}
          </Badge>
        );
      case 'running':
        return (
          <Badge color='gray' variant='light' leftSection={<Loader size={10} />}>
            {tExt('server.planned.running', {})}
          </Badge>
        );
      case 'failed_kept':
        return (
          <Badge color='red' variant='outline'>
            {tExt('server.planned.failedKept', {})}
          </Badge>
        );
      case 'unassigned':
        return (
          <Tooltip label={tExt('server.planned.unassignedTooltip', {})}>
            <Badge color='gray' variant='outline'>
              {tExt('server.planned.unassigned', {})}
            </Badge>
          </Tooltip>
        );
    }
  };

  if (!overview) {
    return (
      <ServerContentContainer title={tExt('server.title', {})}>
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

  return (
    <ServerContentContainer
      title={tExt('server.title', {})}
      contentRight={
        <ServerCan action='backup-plus.manage'>
          <Button
            leftSection={<FontAwesomeIcon icon={faBolt} />}
            loading={running === 'any'}
            disabled={!overview.routines.some((r) => r.source === 'create')}
            onClick={() => doRunNow(null)}
          >
            {tExt('server.button.runNow', {})}
          </Button>
        </ServerCan>
      }
    >
      <Stack>
        {!limits.globallyEnabled && (
          <Alert color='yellow' title={tExt('server.alert.globallyDisabledTitle', {})}>
            {tExt('server.alert.globallyDisabled', {})}
          </Alert>
        )}
        {policy.lastError && (
          <Alert color='red' title={tExt('server.alert.lastRunFailed', {})}>
            {policy.lastError}
          </Alert>
        )}
        {routineErrors.map((r) => (
          <Alert key={r.uuid} color='orange' title={tExt('server.alert.routineError', { name: r.name })}>
            {r.lastError}
          </Alert>
        ))}

        <SimpleGrid cols={{ base: 1, sm: 2, lg: 4 }}>
          <TitleCard title={tExt('server.card.status', {})} icon={faRotate}>
            <Stack gap={4}>
              <Group gap='xs'>
                <Badge color={policy.enabled ? 'green' : 'gray'}>
                  {policy.enabled ? tExt('server.card.active', {}) : tExt('server.card.paused', {})}
                </Badge>
              </Group>
              <Text size='sm' c='dimmed'>
                {tExt('server.card.lastRun', { time: formatDate(policy.lastRun) })}
              </Text>
              <Text size='sm' c='dimmed'>
                {tExt('server.card.timezone', { timezone: overview.timezone })}
              </Text>
            </Stack>
          </TitleCard>
          <TitleCard title={tExt('server.card.managed', {})} icon={faListUl}>
            <Text size='xl' fw={600}>
              {backups.filter((b) => b.planned.type !== 'delete').length}
            </Text>
            <Text size='sm' c='dimmed'>
              {tExt('server.card.used', { size: formatBytes(backups.reduce((sum, b) => sum + b.bytes, 0)) })}
            </Text>
          </TitleCard>
          <TitleCard title={tExt('server.card.limit', {})} icon={faLayerGroup}>
            <Text size='xl' fw={600} c={limitTooLow ? 'red' : undefined}>
              {limits.backupsTotal} / {limits.backupLimit}
            </Text>
            <Text size='sm' c='dimmed'>
              {tExt('server.card.limitHint', {})}
            </Text>
          </TitleCard>
          <TitleCard title={tExt('server.card.routines', {})} icon={faClockRotateLeft}>
            <Stack gap={2}>
              {overview.routines.length === 0 && (
                <Text size='sm' c='dimmed'>
                  {tExt('common.none', {})}
                </Text>
              )}
              {overview.routines.map((r, i) => (
                <Group key={r.uuid} gap={6} wrap='nowrap'>
                  <Badge size='xs' color={routineColor(i)} variant='light'>
                    {r.backupCount}/{r.keepCount}
                  </Badge>
                  <Text size='sm' truncate>
                    {tExt('server.card.routineLine', { name: r.name, next: nextRunLabel(r.enabled, r.nextRun) })}
                  </Text>
                </Group>
              ))}
            </Stack>
          </TitleCard>
        </SimpleGrid>

        <ServerCan action='backup-plus.manage'>
          <TitleCard title={tExt('server.setup.title', {})} icon={faGear}>
            <Stack>
              <Group align='flex-end' wrap='wrap'>
                <Select
                  label={tExt('server.setup.template', {})}
                  description={selectedTemplate?.description}
                  className='flex-1'
                  style={{ minWidth: 260 }}
                  allowDeselect={false}
                  value={templateId}
                  data={overview.templates.map((t) => ({
                    value: t.id,
                    label: t.builtin ? t.name : tExt('server.setup.customTemplate', { name: t.name }),
                  }))}
                  onChange={setTemplateId}
                />
                <Button variant='light' loading={saving} disabled={!templateId} onClick={doApplyTemplate}>
                  {tExt('server.button.applyTemplate', {})}
                </Button>
              </Group>

              <Group>
                <Switch
                  label={tExt('server.setup.enabled', {})}
                  checked={enabled}
                  onChange={(e) => {
                    setEnabled(e.currentTarget.checked);
                    setDirty(true);
                  }}
                />
                <Switch
                  label={tExt('server.setup.deleteFailed', {})}
                  checked={deleteFailed}
                  onChange={(e) => {
                    setDeleteFailed(e.currentTarget.checked);
                    setDirty(true);
                  }}
                />
              </Group>

              {limitTooLow && (
                <Alert color='red'>
                  {tExt('server.alert.limitTooLow', {
                    limit: limits.backupLimit,
                    total: keepTotal,
                    recommended: keepTotal + 1,
                  })}
                </Alert>
              )}
              {!limits.canCustomize && <Alert color='blue'>{tExt('server.alert.customDisabled', {})}</Alert>}
            </Stack>
          </TitleCard>
        </ServerCan>

        <TitleCard title={tExt('server.routines.title', {})} icon={faClockRotateLeft}>
          <Stack>
            <Text size='sm' c='dimmed'>
              {tExt('server.routines.intro', {})}
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
                      {tExt('server.routines.status', {
                        count: saved.backupCount,
                        keep: saved.keepCount,
                        last: formatDate(saved.lastRun),
                        next: nextRunLabel(saved.enabled, saved.nextRun),
                      })}
                    </Text>
                    {saved.source === 'create' && (
                      <ServerCan action='backup-plus.manage'>
                        <Button
                          size='compact-xs'
                          variant='subtle'
                          loading={running === saved.uuid}
                          onClick={() => doRunNow(saved.uuid)}
                        >
                          {tExt('server.button.runRoutine', {})}
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
                  {tExt('server.button.save', {})}
                </Button>
                {dirty && (
                  <Button variant='subtle' onClick={() => applyOverview(overview, true)}>
                    {tExt('server.button.discard', {})}
                  </Button>
                )}
              </Group>
            </ServerCan>
          </Stack>
        </TitleCard>

        <TitleCard title={tExt('server.backups.title', {})} icon={faListUl}>
          {backups.length === 0 ? (
            <Text c='dimmed' size='sm'>
              {tExt('server.backups.empty', {})}
            </Text>
          ) : (
            <Table.ScrollContainer minWidth={760}>
              <Table striped highlightOnHover>
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th>{tExt('server.backups.name', {})}</Table.Th>
                    <Table.Th>{tExt('server.backups.routine', {})}</Table.Th>
                    <Table.Th>{tExt('server.backups.created', {})}</Table.Th>
                    <Table.Th>{tExt('server.backups.size', {})}</Table.Th>
                    <Table.Th>{tExt('server.backups.nextAction', {})}</Table.Th>
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
                              {tExt('common.notAvailable', {})}
                            </Text>
                          )}
                        </Table.Td>
                        <Table.Td>{formatDate(backup.created)}</Table.Td>
                        <Table.Td>
                          {backup.completed ? formatBytes(backup.bytes) : tExt('common.notAvailable', {})}
                        </Table.Td>
                        <Table.Td>{plannedBadge(backup.planned)}</Table.Td>
                        <Table.Td>
                          <ServerCan action='backup-plus.manage'>
                            <Menu position='bottom-end' withinPortal>
                              <Menu.Target>
                                <ActionIcon variant='subtle' aria-label={tExt('server.backups.actions', {})}>
                                  <FontAwesomeIcon icon={faEllipsisVertical} />
                                </ActionIcon>
                              </Menu.Target>
                              <Menu.Dropdown>
                                <Menu.Item
                                  leftSection={<FontAwesomeIcon icon={faThumbtack} />}
                                  onClick={() => doPin(backup)}
                                >
                                  {backup.locked ? tExt('server.backups.unpin', {}) : tExt('server.backups.pin', {})}
                                </Menu.Item>
                                <Menu.Divider />
                                <Menu.Label>{tExt('server.backups.assign', {})}</Menu.Label>
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
                                  {tExt('server.backups.assignNone', {})}
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

        <TitleCard title={tExt('server.log.title', {})} icon={faClockRotateLeft}>
          {log.length === 0 ? (
            <Text c='dimmed' size='sm'>
              {tExt('server.log.empty', {})}
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
