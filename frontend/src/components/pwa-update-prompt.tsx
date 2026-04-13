import { useRegisterSW } from 'virtual:pwa-register/react';

export default function PwaUpdatePrompt() {
  const {
    needRefresh: [needRefresh, setNeedRefresh],
    updateServiceWorker,
  } = useRegisterSW();

  if (!needRefresh) return null;

  return (
    <div className="fixed bottom-4 right-4 z-50 flex items-center gap-3 rounded-lg border border-border bg-card px-4 py-3 shadow-lg">
      <span className="text-sm text-card-foreground">A new version is available.</span>
      <button
        className="rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:bg-primary/90"
        onClick={() => updateServiceWorker(true)}
      >
        Update
      </button>
      <button
        className="rounded-md px-3 py-1.5 text-sm text-muted-foreground hover:text-foreground"
        onClick={() => setNeedRefresh(false)}
      >
        Dismiss
      </button>
    </div>
  );
}
