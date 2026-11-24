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

/** Vorlage im Editor: Routinen mit Schlüsseln statt Namen als Quellverweis. */
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
      minIntervalHours: 1,
      maxKeepTotal: 50,
      maxRoutines: 10,
      allowCustomRoutines: true,
      defaultTemplateId: 'gfs-kompakt',
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
      .then(() => addToast('Einstellungen gespeichert.', 'success'))
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setSaving(false));
  };

  const doSaveTemplates = () => {
    setSaving(true);
    updateTemplates(drafts.map(fromDraft))
      .then((list) => {
        applyTemplates(list);
        addToast('Vorlagen gespeichert.', 'success');
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
      ? { ...toDraft(base), id: `${base.id}-kopie`, name: `${base.name} (Kopie)`, builtin: false }
      : {
          id: `vorlage-${drafts.length + 1}`,
          name: `Eigene Vorlage ${drafts.length + 1}`,
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
    if (
      !window.confirm(
        `Vorlage "${template.name}" auf ${bulkOnlyUnconfigured ? 'alle noch nicht eingerichteten' : 'ALLE'} Server anwenden?` +
          (bulkOnlyUnconfigured ? '' : ' Bestehende Routinen werden ersetzt.'),
      )
    ) {
      return;
    }

    setBulkRunning(true);
    applyTemplateToAll(template.id, bulkOnlyUnconfigured, bulkEnable)
      .then((result) => {
        addToast(
          `Auf ${result.applied} Server angewendet${result.skipped.length > 0 ? `, ${result.skipped.length} übersprungen` : ''}.`,
          result.skipped.length > 0 ? 'warning' : 'success',
        );
        if (result.skipped.length > 0) {
          console.warn('Backup Plus – übersprungene Server:', result.skipped);
        }
        getAdminOverview().then(setOverview).catch(() => {});
      })
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setBulkRunning(false));
  };

  const templateOptions = templates.map((t) => ({
    value: t.id,
    label: t.builtin ? t.name : `${t.name} (eigene)`,
  }));

  return (
    <Stack>
      <TitleCard title='Übersicht' icon={faChartSimple}>
        {overview ? (
          <SimpleGrid cols={{ base: 2, md: 6 }}>
            <Stat label='Server eingerichtet' value={overview.policiesTotal} />
            <Stat label='davon aktiv' value={overview.policiesEnabled} />
            <Stat label='Routinen' value={overview.routinesTotal} />
            <Stat label='mit Fehlern' value={overview.policiesWithErrors} />
            <Stat label='verwaltete Backups' value={overview.managedBackups} />
            <Stat label='Speicher' value={formatBytes(overview.managedBytes)} />
          </SimpleGrid>
        ) : (
          <Text c='dimmed'>Lädt…</Text>
        )}
      </TitleCard>

      <TitleCard title='Globale Einstellungen' icon={faGear}>
        <form onSubmit={form.onSubmit(doSaveSettings)}>
          <Stack>
            <Group>
              <Switch
                label='Backup Plus aktiv'
                description='Hauptschalter für alle Server.'
                {...form.getInputProps('enabled', { type: 'checkbox' })}
              />
              <Switch
                label='Eigene Routinen erlauben'
                description='Aus: Server-Nutzer können nur Vorlagen anwenden (Admins dürfen immer).'
                {...form.getInputProps('allowCustomRoutines', { type: 'checkbox' })}
              />
            </Group>
            <SimpleGrid cols={{ base: 1, sm: 2, lg: 3 }}>
              <Select
                label='Standard-Vorlage'
                description='Wird Servern ohne Einrichtung vorgeschlagen.'
                data={templateOptions}
                allowDeselect={false}
                {...form.getInputProps('defaultTemplateId')}
              />
              <NumberInput
                label='Kleinster Abstand neuer Backups (Stunden)'
                min={1}
                max={168}
                {...form.getInputProps('minIntervalHours')}
              />
              <NumberInput
                label='Max. behaltene Backups pro Server'
                min={1}
                max={500}
                {...form.getInputProps('maxKeepTotal')}
              />
              <NumberInput label='Max. Routinen pro Server' min={1} max={50} {...form.getInputProps('maxRoutines')} />
              <NumberInput
                label='Max. Server pro Minute'
                description='Lastverteilung, wenn viele Server gleichzeitig fällig sind.'
                min={1}
                max={1000}
                {...form.getInputProps('maxBackupsPerTick')}
              />
              <NumberInput
                label='Protokolleinträge pro Server'
                min={10}
                max={10000}
                {...form.getInputProps('logRetention')}
              />
            </SimpleGrid>
            <AdminCan action='extensions.manage' cantSave>
              <Button type='submit' loading={saving} disabled={!form.isValid()} className='w-fit!'>
                Einstellungen speichern
              </Button>
            </AdminCan>
          </Stack>
        </form>
      </TitleCard>

      <TitleCard title='Vorlagen' icon={faLayerGroup}>
        <Stack>
          <Text size='sm' c='dimmed'>
            Eingebaute Vorlagen können nicht geändert, aber als Grundlage kopiert werden. Eigene Vorlagen stehen allen
            Servern zur Auswahl.
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
                        eingebaut
                      </Badge>
                    </Group>
                  </Accordion.Control>
                  <Accordion.Panel>
                    <Stack gap='xs'>
                      <Text size='sm'>{t.description}</Text>
                      {t.routines.map((r) => (
                        <Text key={r.name} size='sm' c='dimmed'>
                          • {r.name}: {r.source === 'create' ? 'erstellt' : `übernimmt aus „${r.sourceName}“`} ·{' '}
                          <code>{r.cron}</code> · behält {r.keepCount}
                          {r.maxAgeHours > 0 ? `, max. ${r.maxAgeHours} h` : ''}
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
                          Als eigene Vorlage kopieren
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
                    <Text fw={500}>{draft.name || '(ohne Namen)'}</Text>
                    <Badge size='xs' color='grape' variant='light'>
                      eigene
                    </Badge>
                  </Group>
                </Accordion.Control>
                <Accordion.Panel>
                  <Stack>
                    <SimpleGrid cols={{ base: 1, sm: 3 }}>
                      <TextInput
                        label='ID'
                        description='Kleinbuchstaben, Ziffern, Bindestriche'
                        value={draft.id}
                        onChange={(e) => updateDraft(index, { id: e.currentTarget.value })}
                      />
                      <TextInput
                        label='Name'
                        value={draft.name}
                        onChange={(e) => updateDraft(index, { name: e.currentTarget.value })}
                      />
                      <TextInput
                        label='Beschreibung'
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
                      Vorlage löschen
                    </Button>
                  </Stack>
                </Accordion.Panel>
              </Accordion.Item>
            ))}
          </Accordion>

          <AdminCan action='extensions.manage' cantSave>
            <Group>
              <Button variant='light' leftSection={<FontAwesomeIcon icon={faPlus} />} onClick={() => addDraft()}>
                Neue Vorlage
              </Button>
              <Button loading={saving} disabled={!templatesDirty} onClick={doSaveTemplates}>
                Vorlagen speichern
              </Button>
            </Group>
          </AdminCan>
        </Stack>
      </TitleCard>

      <TitleCard title='Vorlage auf Server anwenden' icon={faServer}>
        <Stack>
          <Select
            label='Vorlage'
            placeholder='Vorlage wählen'
            data={templateOptions}
            value={bulkTemplate}
            onChange={setBulkTemplate}
          />
          <Group>
            <Switch
              label='Nur Server ohne eigene Routinen'
              checked={bulkOnlyUnconfigured}
              onChange={(e) => setBulkOnlyUnconfigured(e.currentTarget.checked)}
            />
            <Switch
              label='Automatische Backups gleich aktivieren'
              checked={bulkEnable}
              onChange={(e) => setBulkEnable(e.currentTarget.checked)}
            />
          </Group>
          <Text size='sm' c='dimmed'>
            Server, deren Backup-Limit für die Vorlage nicht reicht, werden übersprungen.
          </Text>
          <AdminCan action='extensions.manage'>
            <Button
              className='w-fit!'
              loading={bulkRunning}
              disabled={!bulkTemplate || templatesDirty}
              onClick={doBulkApply}
            >
              Anwenden
            </Button>
          </AdminCan>
        </Stack>
      </TitleCard>
    </Stack>
  );
}
