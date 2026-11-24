import { faClockRotateLeft } from '@fortawesome/free-solid-svg-icons';
import { Extension, ExtensionContext } from 'shared';
import type { IconLike } from '@/elements/data-display/Icon.tsx';
import AdminPage from './AdminPage.tsx';
import CardSummary from './CardSummary.tsx';
import ServerPage from './ServerPage.tsx';

class DevBackupplusGfsbackupsExtension extends Extension {
  public cardConfigurationPage: React.FC | null = AdminPage;
  public cardComponent: React.FC | null = CardSummary;
  public cardPreviewComponent: React.FC | null = null;
  public cardIcon: IconLike | null = faClockRotateLeft;

  public initialize(ctx: ExtensionContext): void {
    ctx.extensionRegistry.enterRoutes((routes) =>
      routes.addServerRoute({
        name: 'Auto-Backups',
        icon: faClockRotateLeft,
        path: '/backup-plus',
        element: ServerPage,
        permission: 'backup-plus.read',
      }),
    );

    ctx.extensionRegistry.permissionIcons.addServerPermissionIcon('backup-plus', faClockRotateLeft);
  }
}

export default new DevBackupplusGfsbackupsExtension();
