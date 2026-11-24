import { z } from 'zod';

export const routineSourceSchema = z.enum(['create', 'promote']);
export type RoutineSource = z.infer<typeof routineSourceSchema>;

export const deleteReasonSchema = z.discriminatedUnion('kind', [
  z.object({ kind: z.literal('failed') }),
  z.object({ kind: z.literal('too_old'), routine: z.string() }),
  z.object({ kind: z.literal('over_keep'), routine: z.string(), keep: z.number() }),
]);
export type DeleteReason = z.infer<typeof deleteReasonSchema>;

export const plannedActionSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('keep'), routine: z.string() }),
  z.object({ type: z.literal('move'), from: z.string(), to: z.string() }),
  z.object({ type: z.literal('delete'), reason: deleteReasonSchema }),
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
  renameTemplate: z.string().nullable(),
  ignoredFiles: z.array(z.string()),
  nextRun: z.coerce.date().nullable(),
  lastRun: z.coerce.date().nullable(),
  lastError: z.string().nullable(),
  backupCount: z.number(),
  throttled: z.boolean().optional(),
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
  renameTemplate: z.string().max(200).nullable(),
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
    minIntervalSeconds: z.number(),
    globalMinIntervalSeconds: z.number(),
    minIntervalOverride: z.number().nullable(),
    isAdmin: z.boolean(),
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
  timezoneSource: z.enum(['server', 'node', 'extension', 'fallback']),
  extensionVersion: z.string().optional(),
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
  renameTemplate: z.string().nullable(),
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
  minIntervalSeconds: z.number().int().min(5).max(604800),
  maxKeepTotal: z.number().int().min(1).max(500),
  maxRoutines: z.number().int().min(1).max(50),
  allowCustomRoutines: z.boolean(),
  defaultTemplateId: z.string().max(64),
  defaultTimezone: z.string().max(64),
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

/** Common schedules (6 fields: second minute hour day month weekday); labels live in translations.ts. */
export const cronPresets = [
  { key: 'everyMinute', value: '0 * * * * *' },
  { key: 'every5m', value: '0 */5 * * * *' },
  { key: 'every15m', value: '0 */15 * * * *' },
  { key: 'every30m', value: '0 */30 * * * *' },
  { key: 'hourly', value: '0 0 * * * *' },
  { key: 'every2h', value: '0 0 */2 * * *' },
  { key: 'every6h', value: '0 0 */6 * * *' },
  { key: 'every12h', value: '0 0 */12 * * *' },
  { key: 'daily0', value: '0 0 0 * * *' },
  { key: 'daily4', value: '0 0 4 * * *' },
  { key: 'weeklyMonday', value: '0 0 0 * * 1' },
  { key: 'weeklySunday', value: '0 0 3 * * 0' },
  { key: 'monthly', value: '0 0 0 1 * *' },
] as const;

export const routineColors = ['blue', 'violet', 'orange', 'teal', 'pink', 'cyan', 'lime', 'grape', 'yellow', 'indigo'];

/** Same short form as the backend: "90 s", "5 min", "2 h", "1 d". */
export function formatInterval(seconds: number): string {
  if (seconds >= 86400 && seconds % 86400 === 0) return `${seconds / 86400} d`;
  if (seconds >= 3600 && seconds % 3600 === 0) return `${seconds / 3600} h`;
  if (seconds >= 60 && seconds % 60 === 0) return `${seconds / 60} min`;
  return `${seconds} s`;
}

export function formatBytes(bytes: number): string {
  if (bytes <= 0) return '0 B';
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** i).toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

export function formatDate(date: Date | null): string {
  if (!date) return '–';
  return date.toLocaleString(undefined, { dateStyle: 'short', timeStyle: 'short' });
}

let keyCounter = 0;
export function newKey(): string {
  keyCounter += 1;
  return `new-${Date.now()}-${keyCounter}`;
}
