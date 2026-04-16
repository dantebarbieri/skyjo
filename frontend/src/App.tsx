import { useEffect } from 'react';
import { Outlet, useLocation, useNavigate } from 'react-router-dom';
import { Toaster } from 'sonner';
import { TooltipProvider } from '@/components/ui/tooltip';
import NavBar from '@/components/nav-bar';
import ConnectionBanner from '@/components/connection-banner';
import PwaUpdatePrompt from '@/components/pwa-update-prompt';
import { useAuth } from '@/contexts/auth-context';
import { initApiClient } from '@/lib/api';

export default function App() {
  const auth = useAuth();
  const location = useLocation();
  const navigate = useNavigate();

  // Initialize the API client with auth callbacks
  useEffect(() => {
    initApiClient(
      () => auth.accessToken,
      () => auth.refresh(),
      () => {
        // Auth failure — do nothing (user will see login button in navbar)
      },
    );
  }, [auth.accessToken, auth.refresh]);

  // Redirect to setup when needed (only if backend is reachable)
  useEffect(() => {
    if (!auth.isLoading && auth.backendAvailable && auth.needsSetup && location.pathname !== '/setup') {
      navigate('/setup', { replace: true });
    }
  }, [auth.isLoading, auth.backendAvailable, auth.needsSetup, location.pathname, navigate]);

  return (
    <TooltipProvider>
      <div className="min-h-screen bg-background text-foreground">
        <NavBar />
        <ConnectionBanner />
        <div className="mx-auto max-w-7xl px-3 py-4 sm:px-4 sm:py-6">
          <Outlet />
        </div>
        <PwaUpdatePrompt />
        <Toaster position="bottom-right" richColors closeButton />
      </div>
    </TooltipProvider>
  );
}
