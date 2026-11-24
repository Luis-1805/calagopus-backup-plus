import { axiosInstance } from '@/api/axios.ts';
import { parseFromApi, serializeForApi } from '@/lib/serialization/api-transform.ts';
import {
  type AdminOverview,
  type AdminSettings,
  adminOverviewSchema,
  adminSettingsSchema,
  type ConfigUpdate,
  configUpdateSchema,
  type Overview,
  overviewSchema,
  type Template,
  templateSchema,
} from './lib/schemas.ts';

const ADMIN_BASE = '/api/admin/extensions/dev.backupplus.gfsbackups';
const serverBase = (uuid: string) => `/api/client/servers/${uuid}/backup-plus`;

export async function getOverview(serverUuid: string): Promise<Overview> {
  const { data } = await axiosInstance.get(serverBase(serverUuid));
  return parseFromApi(overviewSchema, data);
}

export async function updateConfig(serverUuid: string, config: ConfigUpdate): Promise<Overview> {
  const { data } = await axiosInstance.put(serverBase(serverUuid), serializeForApi(configUpdateSchema, config));
  return parseFromApi(overviewSchema, data);
}

export async function applyTemplate(serverUuid: string, templateId: string, enabled: boolean): Promise<Overview> {
  const { data } = await axiosInstance.post(`${serverBase(serverUuid)}/apply-template`, {
    template_id: templateId,
    enabled,
  });
  return parseFromApi(overviewSchema, data);
}

export async function runNow(serverUuid: string, routineUuid: string | null): Promise<void> {
  await axiosInstance.post(`${serverBase(serverUuid)}/run`, { routine_uuid: routineUuid });
}

export async function setPinned(serverUuid: string, backupUuid: string, locked: boolean): Promise<void> {
  await axiosInstance.post(`${serverBase(serverUuid)}/backups/${backupUuid}/pin`, { locked });
}

export async function setBackupRoutine(serverUuid: string, backupUuid: string, routineUuid: string | null): Promise<void> {
  await axiosInstance.put(`${serverBase(serverUuid)}/backups/${backupUuid}/routine`, { routine_uuid: routineUuid });
}

export async function getAdminSettings(): Promise<AdminSettings> {
  const { data } = await axiosInstance.get(`${ADMIN_BASE}/settings`);
  return parseFromApi(adminSettingsSchema, data.settings);
}

export async function updateAdminSettings(settings: AdminSettings): Promise<void> {
  await axiosInstance.put(`${ADMIN_BASE}/settings`, serializeForApi(adminSettingsSchema, settings));
}

export async function getTemplates(): Promise<Template[]> {
  const { data } = await axiosInstance.get(`${ADMIN_BASE}/templates`);
  return data.templates.map((t: unknown) => parseFromApi(templateSchema, t));
}

export async function updateTemplates(templates: Template[]): Promise<Template[]> {
  const { data } = await axiosInstance.put(`${ADMIN_BASE}/templates`, {
    templates: templates.map((t) => serializeForApi(templateSchema, t)),
  });
  return data.templates.map((t: unknown) => parseFromApi(templateSchema, t));
}

export async function applyTemplateToAll(
  templateId: string,
  onlyUnconfigured: boolean,
  enable: boolean,
): Promise<{ applied: number; skipped: string[] }> {
  const { data } = await axiosInstance.post(`${ADMIN_BASE}/apply-template`, {
    template_id: templateId,
    only_unconfigured: onlyUnconfigured,
    enable,
  });
  return data;
}

export async function getAdminOverview(): Promise<AdminOverview> {
  const { data } = await axiosInstance.get(`${ADMIN_BASE}/overview`);
  return parseFromApi(adminOverviewSchema, data.overview);
}
