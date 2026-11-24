import { faClockRotateLeft } from '@fortawesome/free-solid-svg-icons';
import { Extension, ExtensionContext } from 'shared';
import type { IconLike } from '@/elements/data-display/Icon.tsx';
import AdminPage from './AdminPage.tsx';
import CardSummary from './CardSummary.tsx';
import ServerPage from './ServerPage.tsx';
import { getExtTranslations } from './translations.ts';

class DevBackupsplusBackupsExtension extends Extension {
  public cardConfigurationPage: React.FC | null = AdminPage;
  public cardComponent: React.FC | null = CardSummary;
  public cardPreviewComponent: React.FC | null = null;
  public cardIcon: IconLike | null = faClockRotateLeft;

  public initialize(ctx: ExtensionContext): void {
    ctx.extensionRegistry.enterRoutes((routes) =>
      routes.addServerRoute({
        name: () => getExtTranslations().t('server.title', {}),
        icon: faClockRotateLeft,
        path: '/backups-plus',
        element: ServerPage,
        permission: 'backups-plus.read',
      }),
    );

    ctx.extensionRegistry.permissionIcons.addServerPermissionIcon('backups-plus', faClockRotateLeft);
  }
}

export default new DevBackupsplusBackupsExtension();
