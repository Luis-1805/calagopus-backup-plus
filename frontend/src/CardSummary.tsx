import { Text } from '@mantine/core';
import { useEffect, useState } from 'react';
import { getAdminOverview } from './api.ts';
import type { AdminOverview } from './lib/schemas.ts';

export default function CardSummary() {
  const [overview, setOverview] = useState<AdminOverview | null>(null);

  useEffect(() => {
    getAdminOverview()
      .then(setOverview)
      .catch(() => setOverview(null));
  }, []);

  if (!overview) return null;

  return (
    <Text size='sm' c='dimmed'>
      {overview.policiesEnabled} aktive Richtlinien · {overview.managedBackups} Backups
      {overview.policiesWithErrors > 0 ? ` · ${overview.policiesWithErrors} mit Fehlern` : ''}
    </Text>
  );
}
