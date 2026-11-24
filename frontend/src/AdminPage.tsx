import { faChartSimple, faCopy, faGear, faLayerGroup, faPlus, faServer, faTrash } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Accordion, Badge, Group, Select, SimpleGrid, Stack, Text } from '@mantine/core';
import { useForm } from '@mantine/form';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { useEffect, useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import { AdminCan } from '@/elements/Can.tsx';
import Button from '@/elements/buttons/Button.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import NumberInput from '@/elements/input/NumberInput.tsx';
import Switch from '@/elements/input/Switch.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import TimezoneInput from '@/elements/input/TimezoneInput.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import {
  applyTemplateToAll,
  getAdminOverview,
  getAdminSettings,
  getTemplates,
  updateAdminSettings,
  updateTemplates,
} from './api.ts';
import RoutineListEditor, { cleanRoutines, emptyRoutine } from './components/RoutineListEditor.tsx';
import {
  type AdminOverview,
  type AdminSettings,
  adminSettingsSchema,
  formatBytes,
  newKey,
  type RoutineInput,
  type Template,
} from './lib/schemas.ts';
import { useExtTranslations } from './translations.ts';

/** A template in the editor: routines reference their source by key instead of by name. */
interface TemplateDraft {
  id: string;
  name: string;
  description: string;
  builtin: boolean;
  routines: RoutineInput[];
}

function toDraft(template: Template): TemplateDraft {
  const keys = template.routines.map(() => newKey());
  return {
    id: template.id,
    name: template.name,
    description: template.description,
    builtin: template.builtin,
    routines: template.routines.map((r, i) => ({
      key: keys[i],
      uuid: null,
      name: r.name,
      enabled: r.enabled,
      cron: r.cron,
      source: r.source,
      sourceKey: r.sourceName ? (keys[template.routines.findIndex((o) => o.name === r.sourceName)] ?? null) : null,
      keepCount: r.keepCount,
      maxAgeHours: r.maxAgeHours,
      nameTemplate: r.nameTemplate,
      renameTemplate: r.renameTemplate,
      ignoredFiles: r.ignoredFiles,
    })),
  };
}

function fromDraft(draft: TemplateDraft): Template {
  const routines = cleanRoutines(draft.routines);
  return {
    id: draft.id.trim(),
    name: draft.name.trim(),
    description: draft.description.trim(),
    builtin: false,
    routines: routines.map((r) => ({
      name: r.name,
      enabled: r.enabled,
      cron: r.cron,
      source: r.source,
      sourceName: r.source === 'promote' ? (routines.find((o) => o.key === r.sourceKey)?.name ?? null) : null,
      keepCount: r.keepCount,
      maxAgeHours: r.maxAgeHours,
      nameTemplate: r.nameTemplate,
      renameTemplate: r.renameTemplate,
      ignoredFiles: r.ignoredFiles,
    })),
  };
}

function Stat({ label, value }: { label: string; value: string | number }) {
  return (
    <Stack gap={0}>
      <Text size='xl' fw={600}>
        {value}
      </Text>
      <Text size='sm' c='dimmed'>
        {label}
      </Text>
    </Stack>
  );
}

export default function AdminPage() {
  const { addToast } = useToast();
  const { t: tExt } = useExtTranslations();
  const [saving, setSaving] = useState(false);
  const [overview, setOverview] = useState<AdminOverview | null>(null);
  const [templates, setTemplates] = useState<Template[]>([]);
  const [drafts, setDrafts] = useState<TemplateDraft[]>([]);
  const [templatesDirty, setTemplatesDirty] = useState(false);
  const [bulkTemplate, setBulkTemplate] = useState<string | null>(null);
  const [bulkOnlyUnconfigured, setBulkOnlyUnconfigured] = useState(true);
  const [bulkEnable, setBulkEnable] = useState(true);
  const [bulkRunning, setBulkRunning] = useState(false);

  const form = useForm<AdminSettings>({
    initialValues: {
      enabled: true,
      minIntervalMinutes: 60,
      maxKeepTotal: 50,
      maxRoutines: 10,
      allowCustomRoutines: true,
      defaultTemplateId: 'gfs-kompakt',
      defaultTimezone: '',
      maxBackupsPerTick: 10,
      logRetention: 100,
    },
    validateInputOnBlur: true,
    validate: zod4Resolver(adminSettingsSchema),
  });

  const applyTemplates = (list: Template[]) => {
    setTemplates(list);
    setDrafts(list.filter((t) => !t.builtin).map(toDraft));
    setTemplatesDirty(false);
  };

  useEffect(() => {
    getAdminSettings()
      .then((settings) => form.setValues(settings))
      .catch((err) => addToast(httpErrorToHuman(err), 'error'));
    getTemplates()
      .then(applyTemplates)
      .catch((err) => addToast(httpErrorToHuman(err), 'error'));
    getAdminOverview()
      .then(setOverview)
      .catch((err) => addToast(httpErrorToHuman(err), 'error'));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const doSaveSettings = (values: AdminSettings) => {
    setSaving(true);
    updateAdminSettings(values)
      .then(() => addToast(tExt('admin.settings.saved', {}), 'success'))
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setSaving(false));
  };

  const doSaveTemplates = () => {
    setSaving(true);
    updateTemplates(drafts.map(fromDraft))
      .then((list) => {
        applyTemplates(list);
        addToast(tExt('admin.templates.saved', {}), 'success');
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setSaving(false));
  };

  const updateDraft = (index: number, patch: Partial<TemplateDraft>) => {
    setDrafts((current) => current.map((d, i) => (i === index ? { ...d, ...patch } : d)));
    setTemplatesDirty(true);
  };

  const addDraft = (base?: Template) => {
    const draft: TemplateDraft = base
      ? {
          ...toDraft(base),
          id: `${base.id}-copy`,
          name: tExt('admin.templates.copySuffix', { name: base.name }),
          builtin: false,
        }
      : {
          id: `template-${drafts.length + 1}`,
          name: tExt('admin.templates.defaultName', { number: drafts.length + 1 }),
          description: '',
          builtin: false,
          routines: [emptyRoutine(0)],
        };
    setDrafts((current) => [...current, draft]);
    setTemplatesDirty(true);
  };

  const doBulkApply = () => {
    const template = templates.find((t) => t.id === bulkTemplate);
    if (!template) return;
    const question = bulkOnlyUnconfigured
      ? tExt('admin.bulk.confirmUnconfigured', { name: template.name })
      : tExt('admin.bulk.confirmAll', { name: template.name });
    if (!window.confirm(question)) {
      return;
    }

    setBulkRunning(true);
    applyTemplateToAll(template.id, bulkOnlyUnconfigured, bulkEnable)
      .then((result) => {
        if (result.skipped.length > 0) {
          addToast(
            tExt('admin.bulk.resultSkipped', { applied: result.applied, skipped: result.skipped.length }),
            'warning',
          );
          console.warn('Backups Plus – skipped servers:', result.skipped);
        } else {
          addToast(tExt('admin.bulk.result', { applied: result.applied }), 'success');
        }
        getAdminOverview()
          .then(setOverview)
          .catch(() => {});
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setBulkRunning(false));
  };

  const templateOptions = templates.map((t) => ({
    value: t.id,
    label: t.builtin ? t.name : `${t.name} (${tExt('admin.templates.custom', {})})`,
  }));

  return (
    <Stack>
      <TitleCard title={tExt('admin.overview.title', {})} icon={faChartSimple}>
        {overview ? (
          <SimpleGrid cols={{ base: 2, md: 6 }}>
            <Stat label={tExt('admin.overview.servers', {})} value={overview.policiesTotal} />
            <Stat label={tExt('admin.overview.active', {})} value={overview.policiesEnabled} />
            <Stat label={tExt('admin.overview.routines', {})} value={overview.routinesTotal} />
            <Stat label={tExt('admin.overview.errors', {})} value={overview.policiesWithErrors} />
            <Stat label={tExt('admin.overview.backups', {})} value={overview.managedBackups} />
            <Stat label={tExt('admin.overview.storage', {})} value={formatBytes(overview.managedBytes)} />
          </SimpleGrid>
        ) : (
          <Text c='dimmed'>{tExt('admin.overview.loading', {})}</Text>
        )}
      </TitleCard>

      <TitleCard title={tExt('admin.settings.title', {})} icon={faGear}>
        <form onSubmit={form.onSubmit(doSaveSettings)}>
          <Stack>
            <Group>
              <Switch
                label={tExt('admin.settings.enabled', {})}
                description={tExt('admin.settings.enabledDescription', {})}
                {...form.getInputProps('enabled', { type: 'checkbox' })}
              />
              <Switch
                label={tExt('admin.settings.allowCustom', {})}
                description={tExt('admin.settings.allowCustomDescription', {})}
                {...form.getInputProps('allowCustomRoutines', { type: 'checkbox' })}
              />
            </Group>
            <SimpleGrid cols={{ base: 1, sm: 2, lg: 3 }}>
              <Select
                label={tExt('admin.settings.defaultTemplate', {})}
                description={tExt('admin.settings.defaultTemplateDescription', {})}
                data={templateOptions}
                allowDeselect={false}
                {...form.getInputProps('defaultTemplateId')}
              />
              <TimezoneInput
                label={tExt('admin.settings.defaultTimezone', {})}
                description={tExt('admin.settings.defaultTimezoneDescription', {})}
                placeholder='UTC'
                clearable
                value={form.values.defaultTimezone || null}
                onChange={(value) => form.setFieldValue('defaultTimezone', value ?? '')}
              />
              <NumberInput
                label={tExt('admin.settings.minInterval', {})}
                description={tExt('admin.settings.minIntervalDescription', {})}
                min={5}
                max={10080}
                step={5}
                {...form.getInputProps('minIntervalMinutes')}
              />
              <NumberInput
                label={tExt('admin.settings.maxKeep', {})}
                min={1}
                max={500}
                {...form.getInputProps('maxKeepTotal')}
              />
              <NumberInput
                label={tExt('admin.settings.maxRoutines', {})}
                min={1}
                max={50}
                {...form.getInputProps('maxRoutines')}
              />
              <NumberInput
                label={tExt('admin.settings.perTick', {})}
                description={tExt('admin.settings.perTickDescription', {})}
                min={1}
                max={1000}
                {...form.getInputProps('maxBackupsPerTick')}
              />
              <NumberInput
                label={tExt('admin.settings.logRetention', {})}
                min={10}
                max={10000}
                {...form.getInputProps('logRetention')}
              />
            </SimpleGrid>
            <AdminCan action='extensions.manage' cantSave>
              <Button type='submit' loading={saving} disabled={!form.isValid()} className='w-fit!'>
                {tExt('admin.settings.save', {})}
              </Button>
            </AdminCan>
          </Stack>
        </form>
      </TitleCard>

      <TitleCard title={tExt('admin.templates.title', {})} icon={faLayerGroup}>
        <Stack>
          <Text size='sm' c='dimmed'>
            {tExt('admin.templates.intro', {})}
          </Text>

          <Accordion variant='separated'>
            {templates
              .filter((t) => t.builtin)
              .map((t) => (
                <Accordion.Item key={t.id} value={t.id}>
                  <Accordion.Control>
                    <Group gap='xs'>
                      <Text fw={500}>{t.name}</Text>
                      <Badge size='xs' variant='light'>
                        {tExt('admin.templates.builtin', {})}
                      </Badge>
                    </Group>
                  </Accordion.Control>
                  <Accordion.Panel>
                    <Stack gap='xs'>
                      <Text size='sm'>{t.description}</Text>
                      {t.routines.map((r) => (
                        <Text key={r.name} size='sm' c='dimmed'>
                          {tExt('admin.templates.routineLine', {
                            name: r.name,
                            kind:
                              r.source === 'create'
                                ? tExt('admin.templates.routineCreate', {})
                                : tExt('admin.templates.routinePromote', { source: r.sourceName ?? '?' }),
                            cron: r.cron,
                            keep: r.keepCount,
                          })}
                          {r.maxAgeHours > 0 ? tExt('admin.templates.routineMaxAge', { hours: r.maxAgeHours }) : ''}
                        </Text>
                      ))}
                      <AdminCan action='extensions.manage'>
                        <Button
                          size='xs'
                          variant='light'
                          className='w-fit!'
                          leftSection={<FontAwesomeIcon icon={faCopy} />}
                          onClick={() => addDraft(t)}
                        >
                          {tExt('admin.templates.copy', {})}
                        </Button>
                      </AdminCan>
                    </Stack>
                  </Accordion.Panel>
                </Accordion.Item>
              ))}

            {drafts.map((draft, index) => (
              <Accordion.Item key={index} value={`custom-${index}`}>
                <Accordion.Control>
                  <Group gap='xs'>
                    <Text fw={500}>{draft.name || tExt('common.unnamed', {})}</Text>
                    <Badge size='xs' color='grape' variant='light'>
                      {tExt('admin.templates.custom', {})}
                    </Badge>
                  </Group>
                </Accordion.Control>
                <Accordion.Panel>
                  <Stack>
                    <SimpleGrid cols={{ base: 1, sm: 3 }}>
                      <TextInput
                        label={tExt('admin.templates.id', {})}
                        description={tExt('admin.templates.idDescription', {})}
                        value={draft.id}
                        onChange={(e) => updateDraft(index, { id: e.currentTarget.value })}
                      />
                      <TextInput
                        label={tExt('admin.templates.name', {})}
                        value={draft.name}
                        onChange={(e) => updateDraft(index, { name: e.currentTarget.value })}
                      />
                      <TextInput
                        label={tExt('admin.templates.description', {})}
                        value={draft.description}
                        onChange={(e) => updateDraft(index, { description: e.currentTarget.value })}
                      />
                    </SimpleGrid>
                    <RoutineListEditor
                      routines={draft.routines}
                      onChange={(routines) => updateDraft(index, { routines })}
                      timezone='UTC'
                      maxRoutines={form.values.maxRoutines}
                    />
                    <Button
                      color='red'
                      variant='subtle'
                      className='w-fit!'
                      leftSection={<FontAwesomeIcon icon={faTrash} />}
                      onClick={() => {
                        setDrafts((current) => current.filter((_, i) => i !== index));
                        setTemplatesDirty(true);
                      }}
                    >
                      {tExt('admin.templates.delete', {})}
                    </Button>
                  </Stack>
                </Accordion.Panel>
              </Accordion.Item>
            ))}
          </Accordion>

          <AdminCan action='extensions.manage' cantSave>
            <Group>
              <Button variant='light' leftSection={<FontAwesomeIcon icon={faPlus} />} onClick={() => addDraft()}>
                {tExt('admin.templates.add', {})}
              </Button>
              <Button loading={saving} disabled={!templatesDirty} onClick={doSaveTemplates}>
                {tExt('admin.templates.save', {})}
              </Button>
            </Group>
          </AdminCan>
        </Stack>
      </TitleCard>

      <TitleCard title={tExt('admin.bulk.title', {})} icon={faServer}>
        <Stack>
          <Select
            label={tExt('admin.bulk.template', {})}
            placeholder={tExt('admin.bulk.placeholder', {})}
            data={templateOptions}
            value={bulkTemplate}
            onChange={setBulkTemplate}
          />
          <Group>
            <Switch
              label={tExt('admin.bulk.onlyUnconfigured', {})}
              checked={bulkOnlyUnconfigured}
              onChange={(e) => setBulkOnlyUnconfigured(e.currentTarget.checked)}
            />
            <Switch
              label={tExt('admin.bulk.enable', {})}
              checked={bulkEnable}
              onChange={(e) => setBulkEnable(e.currentTarget.checked)}
            />
          </Group>
          <Text size='sm' c='dimmed'>
            {tExt('admin.bulk.hint', {})}
          </Text>
          <AdminCan action='extensions.manage'>
            <Button
              className='w-fit!'
              loading={bulkRunning}
              disabled={!bulkTemplate || templatesDirty}
              onClick={doBulkApply}
            >
              {tExt('admin.bulk.apply', {})}
            </Button>
          </AdminCan>
        </Stack>
      </TitleCard>
    </Stack>
  );
}
