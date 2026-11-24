import { z } from 'zod';

export const tierSchema = z.enum(['hourly', 'daily', 'weekly', 'monthly']);
export type Tier = z.infer<typeof tierSchema>;

export const tierLabels: Record<Tier, string> = {
  hourly: 'Stündlich',
  daily: 'Täglich',
  weekly: 'Wöchentlich',
  monthly: 'Monatlich',
};

export const tierColors: Record<Tier, string> = {
  hourly: 'blue',
  daily: 'teal',
  weekly: 'violet',
  monthly: 'orange',
};

export const plannedActionSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('keep'), tier: tierSchema }),
  z.object({ type: z.literal('promote'), from: tierSchema, to: tierSchema }),
  z.object({ type: z.literal('delete'), reason: z.string() }),
  z.object({ type: z.literal('pinned') }),
  z.object({ type: z.literal('running') }),
  z.object({ type: z.literal('failed_kept') }),
]);
export type PlannedAction = z.infer<typeof plannedActionSchema>;

export const policySchema = z.object({
  exists: z.boolean(),
  enabled: z.boolean(),
  intervalHours: z.number(),
  minuteOffset: z.number(),
  keepHourly: z.number(),
  keepDaily: z.number(),
  keepWeekly: z.number(),
  keepMonthly: z.number(),
  nameTemplate: z.string(),
  ignoredFiles: z.array(z.string()),
  deleteFailed: z.boolean(),
  backupGroupUuid: z.string().nullable(),
  nextRun: z.coerce.date().nullable(),
  lastRun: z.coerce.date().nullable(),
  lastError: z.string().nullable(),
});
export type Policy = z.infer<typeof policySchema>;

export const managedBackupSchema = z.object({
  uuid: z.string(),
  name: z.string(),
  tier: tierSchema,
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
export type LogEntry = z.infer<typeof logEntrySchema>;

export const limitsSchema = z.object({
  globallyEnabled: z.boolean(),
  minIntervalHours: z.number(),
  maxKeepTotal: z.number(),
  backupLimit: z.number(),
  backupsTotal: z.number(),
});

export const overviewSchema = z.object({
  policy: policySchema,
  backups: z.array(managedBackupSchema),
  log: z.array(logEntrySchema),
  limits: limitsSchema,
});
export type Overview = z.infer<typeof overviewSchema>;

export const policyUpdateSchema = z.object({
  enabled: z.boolean(),
  intervalHours: z.number().int().min(1).max(168),
  minuteOffset: z.number().int().min(0).max(59),
  keepHourly: z.number().int().min(1).max(100),
  keepDaily: z.number().int().min(0).max(100),
  keepWeekly: z.number().int().min(0).max(100),
  keepMonthly: z.number().int().min(0).max(100),
  nameTemplate: z.string().min(1).max(200),
  ignoredFiles: z.array(z.string()),
  deleteFailed: z.boolean(),
});
export type PolicyUpdate = z.infer<typeof policyUpdateSchema>;

export const adminSettingsSchema = z.object({
  enabled: z.boolean(),
  minIntervalHours: z.number().int().min(1).max(168),
  maxKeepTotal: z.number().int().min(1).max(500),
  maxBackupsPerTick: z.number().int().min(1).max(1000),
  logRetention: z.number().int().min(10).max(10000),
});
export type AdminSettings = z.infer<typeof adminSettingsSchema>;

export const adminOverviewSchema = z.object({
  policiesTotal: z.number(),
  policiesEnabled: z.number(),
  managedBackups: z.number(),
  managedBytes: z.number(),
  policiesWithErrors: z.number(),
});
export type AdminOverview = z.infer<typeof adminOverviewSchema>;

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
