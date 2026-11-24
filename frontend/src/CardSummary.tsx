import { Text } from '@mantine/core';
import { useEffect, useState } from 'react';
import { getAdminOverview } from './api.ts';
import type { AdminOverview } from './lib/schemas.ts';
import { useExtTranslations } from './translations.ts';

export default function CardSummary() {
  const { t: tExt } = useExtTranslations();
  const [overview, setOverview] = useState<AdminOverview | null>(null);

  useEffect(() => {
    getAdminOverview()
      .then(setOverview)
      .catch(() => setOverview(null));
  }, []);

  if (!overview) return null;

  return (
    <Text size='sm' c='dimmed'>
      {tExt('card.summary', {
        servers: overview.policiesEnabled,
        routines: overview.routinesTotal,
        backups: overview.managedBackups,
      })}
      {overview.policiesWithErrors > 0 ? tExt('card.errors', { count: overview.policiesWithErrors }) : ''}
    </Text>
  );
}
