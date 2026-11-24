import { axiosInstance } from '@/api/axios.ts';
import { parseFromApi, serializeForApi } from '@/lib/serialization/api-transform.ts';
import {
  type AdminOverview,
  type AdminSettings,
  adminOverviewSchema,
  adminSettingsSchema,
  type Overview,
  overviewSchema,
  type PolicyUpdate,
  policyUpdateSchema,
  type Tier,
} from './lib/schemas.ts';

const ADMIN_BASE = '/api/admin/extensions/dev.backupplus.gfsbackups';
const serverBase = (uuid: string) => `/api/client/servers/${uuid}/backup-plus`;

export async function getOverview(serverUuid: string): Promise<Overview> {
  const { data } = await axiosInstance.get(serverBase(serverUuid));
  return parseFromApi(overviewSchema, data);
}

export async function updatePolicy(serverUuid: string, policy: PolicyUpdate): Promise<Overview> {
  const { data } = await axiosInstance.put(serverBase(serverUuid), serializeForApi(policyUpdateSchema, policy));
  return parseFromApi(overviewSchema, data);
}

export async function runNow(serverUuid: string): Promise<void> {
  await axiosInstance.post(`${serverBase(serverUuid)}/run`);
}

export async function setPinned(serverUuid: string, backupUuid: string, locked: boolean): Promise<void> {
  await axiosInstance.post(`${serverBase(serverUuid)}/backups/${backupUuid}/pin`, { locked });
}

export async function setTier(serverUuid: string, backupUuid: string, tier: Tier): Promise<void> {
  await axiosInstance.put(`${serverBase(serverUuid)}/backups/${backupUuid}/tier`, { tier });
}

export async function getAdminSettings(): Promise<AdminSettings> {
  const { data } = await axiosInstance.get(`${ADMIN_BASE}/settings`);
  return parseFromApi(adminSettingsSchema, data.settings);
}

export async function updateAdminSettings(settings: AdminSettings): Promise<void> {
  await axiosInstance.put(`${ADMIN_BASE}/settings`, serializeForApi(adminSettingsSchema, settings));
}

export async function getAdminOverview(): Promise<AdminOverview> {
  const { data } = await axiosInstance.get(`${ADMIN_BASE}/overview`);
  return parseFromApi(adminOverviewSchema, data.overview);
}
