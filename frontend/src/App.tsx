import { Outlet } from 'react-router-dom';
import { TooltipProvider } from '@/components/ui/tooltip';
import NavBar from '@/components/nav-bar';
import PwaUpdatePrompt from '@/components/pwa-update-prompt';

export default function App() {
  return (
    <TooltipProvider>
      <div className="min-h-screen bg-background text-foreground">
        <NavBar />
        <div className="mx-auto max-w-7xl px-4 py-6">
          <Outlet />
        </div>
        <PwaUpdatePrompt />
      </div>
    </TooltipProvider>
  );
}
