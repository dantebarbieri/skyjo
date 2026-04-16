import { useRef, useState } from 'react';
import { useAuth, type ConnectivityStatus } from '@/contexts/auth-context';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

const bannerConfig: Record<
  Exclude<ConnectivityStatus, 'online'>,
  { icon: string; message: string; detail: string; showRetry: boolean; colorClasses: string }
> = {
  'client-offline': {
    icon: '📡',
    message: 'You are offline',
    detail: 'Local play and simulation still work.',
    showRetry: false,
    colorClasses: 'bg-blue-50 text-blue-900 border-blue-200 dark:bg-blue-950 dark:text-blue-100 dark:border-blue-800',
  },
  'server-unreachable': {
    icon: '⚠️',
    message: 'Server unreachable',
    detail: 'Online features are unavailable.',
    showRetry: true,
    colorClasses: 'bg-amber-50 text-amber-900 border-amber-200 dark:bg-amber-950 dark:text-amber-100 dark:border-amber-800',
  },
  'database-degraded': {
    icon: '⚠️',
    message: 'Server database issue',
    detail: 'Some features may not work.',
    showRetry: true,
    colorClasses: 'bg-orange-50 text-orange-900 border-orange-200 dark:bg-orange-950 dark:text-orange-100 dark:border-orange-800',
  },
};

export default function ConnectionBanner() {
  const { connectivityStatus, retryConnection } = useAuth();
  const [dismissed, setDismissed] = useState(false);
  const [retrying, setRetrying] = useState(false);
  const prevStatusRef = useRef(connectivityStatus);

  // Reset dismissed state when connectivity status changes (so future outages surface)
  if (connectivityStatus !== prevStatusRef.current) {
    prevStatusRef.current = connectivityStatus;
    if (connectivityStatus !== 'online') {
      setDismissed(false);
    }
  }

  // Nothing to show when online or dismissed
  if (connectivityStatus === 'online' || dismissed) return null;

  const config = bannerConfig[connectivityStatus];

  const handleRetry = async () => {
    setRetrying(true);
    retryConnection();
    // Allow a brief delay so the user sees the spinner before the status may update
    setTimeout(() => setRetrying(false), 2000);
  };

  const handleDismiss = () => {
    setDismissed(true);
  };

  return (
    <div
      className={cn(
        'sticky top-14 z-20 w-full border-b px-4 py-2',
        config.colorClasses,
      )}
      role="alert"
    >
      <div className="mx-auto max-w-7xl flex flex-wrap items-center gap-2 text-sm">
        <span className="shrink-0">{config.icon}</span>
        <span className="font-medium">{config.message}</span>
        <span className="hidden sm:inline">—</span>
        <span>{config.detail}</span>
        <div className="ml-auto flex items-center gap-1 shrink-0">
          {config.showRetry && (
            <Button
              variant="outline"
              size="sm"
              className="min-h-[44px] min-w-[44px] text-xs"
              onClick={handleRetry}
              disabled={retrying}
            >
              {retrying ? (
                <span className="flex items-center gap-1">
                  <span className="h-3 w-3 animate-spin rounded-full border-2 border-current border-t-transparent" />
                  Retrying…
                </span>
              ) : (
                'Retry'
              )}
            </Button>
          )}
          <Button
            variant="ghost"
            size="sm"
            className="min-h-[44px] min-w-[44px] text-xs"
            onClick={handleDismiss}
          >
            Dismiss
          </Button>
        </div>
      </div>
    </div>
  );
}
