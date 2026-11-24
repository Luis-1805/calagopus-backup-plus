import { faGear, faChartSimple } from '@fortawesome/free-solid-svg-icons';
import { SimpleGrid, Stack, Text } from '@mantine/core';
import { useForm } from '@mantine/form';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { useEffect, useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import { AdminCan } from '@/elements/Can.tsx';
import Button from '@/elements/buttons/Button.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import NumberInput from '@/elements/input/NumberInput.tsx';
import Switch from '@/elements/input/Switch.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { getAdminOverview, getAdminSettings, updateAdminSettings } from './api.ts';
import { type AdminOverview, type AdminSettings, adminSettingsSchema, formatBytes } from './lib/schemas.ts';

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

  const form = useForm<AdminSettings>({
    initialValues: {
      enabled: true,
      minIntervalHours: 1,
      maxKeepTotal: 24,
      maxBackupsPerTick: 10,
      logRetention: 100,
    },
    validateInputOnBlur: true,
    validate: zod4Resolver(adminSettingsSchema),
  });

  useEffect(() => {
    getAdminSettings()
      .then((settings) => form.setValues(settings))
      .catch((err) => addToast(httpErrorToHuman(err), 'error'));
    getAdminOverview()
      .then(setOverview)
      .catch((err) => addToast(httpErrorToHuman(err), 'error'));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const doSave = (values: AdminSettings) => {
    setSaving(true);
    updateAdminSettings(values)
      .then(() => addToast('Einstellungen gespeichert.', 'success'))
      .catch((err) => addToast(httpErrorToHuman(err), 'error'))
      .finally(() => setSaving(false));
  };

  return (
    <Stack>
      <TitleCard title='Übersicht' icon={faChartSimple}>
        {overview ? (
          <SimpleGrid cols={{ base: 2, md: 5 }}>
            <Stat label='Richtlinien' value={overview.policiesTotal} />
            <Stat label='davon aktiv' value={overview.policiesEnabled} />
            <Stat label='mit Fehlern' value={overview.policiesWithErrors} />
            <Stat label='verwaltete Backups' value={overview.managedBackups} />
            <Stat label='Speicher' value={formatBytes(overview.managedBytes)} />
          </SimpleGrid>
        ) : (
          <Text c='dimmed'>Lädt…</Text>
        )}
      </TitleCard>

      <TitleCard title='Globale Einstellungen' icon={faGear}>
        <form onSubmit={form.onSubmit(doSave)}>
          <Stack>
            <Switch
              label='Backup Plus aktiv'
              description='Hauptschalter. Ausgeschaltet werden keine automatischen Backups erstellt oder gelöscht.'
              {...form.getInputProps('enabled', { type: 'checkbox' })}
            />
            <SimpleGrid cols={{ base: 1, sm: 2 }}>
              <NumberInput
                label='Kleinstes erlaubtes Intervall (Stunden)'
                min={1}
                max={168}
                {...form.getInputProps('minIntervalHours')}
              />
              <NumberInput
                label='Maximal behaltene Backups pro Server'
                min={1}
                max={500}
                {...form.getInputProps('maxKeepTotal')}
              />
              <NumberInput
                label='Max. neue Backups pro Minute (panelweit)'
                description='Verteilt die Last, wenn viele Server zur selben Zeit fällig sind.'
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
            <Text size='sm' c='dimmed'>
              Tipp: Jeder Server braucht ein Backup-Limit von mindestens der Summe der behaltenen Backups (bei der
              Standard-Richtlinie 5, empfohlen 6).
            </Text>
            <AdminCan action='extensions.manage' cantSave>
              <Button type='submit' loading={saving} disabled={!form.isValid()} className='w-fit!'>
                Speichern
              </Button>
            </AdminCan>
          </Stack>
        </form>
      </TitleCard>
    </Stack>
  );
}
