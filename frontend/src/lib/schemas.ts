import { z } from 'zod';

export const routineSourceSchema = z.enum(['create', 'promote']);
export type RoutineSource = z.infer<typeof routineSourceSchema>;

export const plannedActionSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('keep'), routine: z.string() }),
  z.object({ type: z.literal('move'), from: z.string(), to: z.string() }),
  z.object({ type: z.literal('delete'), reason: z.string() }),
  z.object({ type: z.literal('pinned') }),
  z.object({ type: z.literal('running') }),
  z.object({ type: z.literal('failed_kept') }),
  z.object({ type: z.literal('unassigned') }),
]);
export type PlannedAction = z.infer<typeof plannedActionSchema>;

export const policySchema = z.object({
  exists: z.boolean(),
  enabled: z.boolean(),
  deleteFailed: z.boolean(),
  lastRun: z.coerce.date().nullable(),
  lastError: z.string().nullable(),
});

export const routineSchema = z.object({
  uuid: z.string(),
  name: z.string(),
  enabled: z.boolean(),
  position: z.number(),
  cron: z.string(),
  source: routineSourceSchema,
  sourceRoutineUuid: z.string().nullable(),
  keepCount: z.number(),
  maxAgeHours: z.number(),
  nameTemplate: z.string(),
  ignoredFiles: z.array(z.string()),
  nextRun: z.coerce.date().nullable(),
  lastRun: z.coerce.date().nullable(),
  lastError: z.string().nullable(),
  backupCount: z.number(),
});
export type Routine = z.infer<typeof routineSchema>;

export const managedBackupSchema = z.object({
  uuid: z.string(),
  name: z.string(),
  routineUuid: z.string().nullable(),
  successful: z.boolean(),
  locked: z.boolean(),
  bytes: z.number(),
  completed: z.coerce.date().nullable(),
  created: z.coerce.date(),
  planned: plannedActionSchema,
});
export type ManagedBackup = z.infer<typeof managedBackupSchema>;

export const logEntrySchema = z.object({
  level: z.string(),
  message: z.string(),
  created: z.coerce.date(),
});

export const templateRoutineSchema = z.object({
  name: z.string().min(1).max(64),
  enabled: z.boolean(),
  cron: z.string().min(1).max(128),
  source: routineSourceSchema,
  sourceName: z.string().nullable(),
  keepCount: z.number().int().min(1).max(500),
  maxAgeHours: z.number().int().min(0).max(87600),
  nameTemplate: z.string().min(1).max(200),
  ignoredFiles: z.array(z.string()),
});
export type TemplateRoutine = z.infer<typeof templateRoutineSchema>;

export const templateSchema = z.object({
  id: z.string().min(1).max(64),
  name: z.string().min(1).max(100),
  description: z.string().max(500),
  builtin: z.boolean(),
  routines: z.array(templateRoutineSchema),
});
export type Template = z.infer<typeof templateSchema>;

export const overviewSchema = z.object({
  policy: policySchema,
  routines: z.array(routineSchema),
  backups: z.array(managedBackupSchema),
  log: z.array(logEntrySchema),
  limits: z.object({
    globallyEnabled: z.boolean(),
    minIntervalHours: z.number(),
    maxKeepTotal: z.number(),
    maxRoutines: z.number(),
    allowCustomRoutines: z.boolean(),
    canCustomize: z.boolean(),
    backupLimit: z.number(),
    backupsTotal: z.number(),
  }),
  templates: z.array(templateSchema),
  defaultTemplateId: z.string(),
  timezone: z.string(),
});
export type Overview = z.infer<typeof overviewSchema>;

export const routineInputSchema = z.object({
  key: z.string(),
  uuid: z.string().nullable(),
  name: z.string(),
  enabled: z.boolean(),
  cron: z.string(),
  source: routineSourceSchema,
  sourceKey: z.string().nullable(),
  keepCount: z.number(),
  maxAgeHours: z.number(),
  nameTemplate: z.string(),
  ignoredFiles: z.array(z.string()),
});
export type RoutineInput = z.infer<typeof routineInputSchema>;

export const configUpdateSchema = z.object({
  enabled: z.boolean(),
  deleteFailed: z.boolean(),
  routines: z.array(routineInputSchema),
});
export type ConfigUpdate = z.infer<typeof configUpdateSchema>;

export const adminSettingsSchema = z.object({
  enabled: z.boolean(),
  minIntervalHours: z.number().int().min(1).max(168),
  maxKeepTotal: z.number().int().min(1).max(500),
  maxRoutines: z.number().int().min(1).max(50),
  allowCustomRoutines: z.boolean(),
  defaultTemplateId: z.string().max(64),
  maxBackupsPerTick: z.number().int().min(1).max(1000),
  logRetention: z.number().int().min(10).max(10000),
});
export type AdminSettings = z.infer<typeof adminSettingsSchema>;

export const adminOverviewSchema = z.object({
  policiesTotal: z.number(),
  policiesEnabled: z.number(),
  routinesTotal: z.number(),
  managedBackups: z.number(),
  managedBytes: z.number(),
  policiesWithErrors: z.number(),
});
export type AdminOverview = z.infer<typeof adminOverviewSchema>;

/** Häufige Zeitpläne (6 Felder: Sekunde Minute Stunde Tag Monat Wochentag). */
export const cronPresets: { label: string; value: string }[] = [
  { label: 'Stündlich (zur vollen Stunde)', value: '0 0 * * * *' },
  { label: 'Alle 2 Stunden', value: '0 0 */2 * * *' },
  { label: 'Alle 6 Stunden', value: '0 0 */6 * * *' },
  { label: 'Alle 12 Stunden', value: '0 0 */12 * * *' },
  { label: 'Täglich um 00:00', value: '0 0 0 * * *' },
  { label: 'Täglich um 04:00', value: '0 0 4 * * *' },
  { label: 'Wöchentlich, Montag 00:00', value: '0 0 0 * * 1' },
  { label: 'Wöchentlich, Sonntag 03:00', value: '0 0 3 * * 0' },
  { label: 'Monatlich, am 1. um 00:00', value: '0 0 0 1 * *' },
];

export const routineColors = ['blue', 'violet', 'orange', 'teal', 'pink', 'cyan', 'lime', 'grape', 'yellow', 'indigo'];

export function formatBytes(bytes: number): string {
  if (bytes <= 0) return '0 B';
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** i).toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

export function formatDate(date: Date | null): string {
  if (!date) return '–';
  return date.toLocaleString('de-DE', { dateStyle: 'short', timeStyle: 'short' });
}

let keyCounter = 0;
export function newKey(): string {
  keyCounter += 1;
  return `new-${Date.now()}-${keyCounter}`;
}
