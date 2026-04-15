import { NavLink, useNavigate } from 'react-router-dom';
import { cn } from '@/lib/utils';
import { useAuth } from '@/contexts/auth-context';
import { Button } from '@/components/ui/button';
import { Trophy } from 'lucide-react';

const links = [
  { to: '/rules', label: 'Rules' },
  { to: '/play', label: 'Play' },
  { to: '/simulator', label: 'Simulator' },
  { to: '/leaderboard', label: 'Leaderboard', icon: Trophy },
];

export default function NavBar() {
  const { user, isAuthenticated, backendAvailable, logout } = useAuth();
  const navigate = useNavigate();

  return (
    <nav className="border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
      <div className="mx-auto max-w-7xl px-4 flex items-center h-14 gap-3 sm:gap-6">
        <NavLink to="/" className="text-lg sm:text-xl font-bold tracking-tight">
          Skyjo
        </NavLink>
        <div className="flex items-center gap-1">
          {links.map(({ to, label, icon: Icon }) => (
            <NavLink
              key={to}
              to={to}
              className={({ isActive }) =>
                cn(
                  'px-2 py-1 sm:px-3 sm:py-1.5 rounded-md text-xs sm:text-sm font-medium transition-colors inline-flex items-center gap-1',
                  isActive
                    ? 'bg-accent text-accent-foreground'
                    : 'text-muted-foreground hover:text-foreground hover:bg-accent/50'
                )
              }
            >
              {Icon && <Icon className="h-3.5 w-3.5" />}
              {label}
            </NavLink>
          ))}
        </div>

        {/* Auth section — pushed to the right */}
        <div className="ml-auto flex items-center gap-2">
          {backendAvailable && isAuthenticated && user && (
            <>
              <NavLink
                to="/settings"
                className="text-xs sm:text-sm text-muted-foreground hover:text-foreground transition-colors"
              >
                {user.display_name}
              </NavLink>
              {user.permission === 'admin' && (
                <NavLink
                  to="/admin"
                  className={({ isActive }) =>
                    cn(
                      'px-2 py-1 rounded-md text-xs sm:text-sm font-medium transition-colors',
                      isActive
                        ? 'bg-accent text-accent-foreground'
                        : 'text-muted-foreground hover:text-foreground hover:bg-accent/50'
                    )
                  }
                >
                  Admin
                </NavLink>
              )}
              <Button
                variant="ghost"
                size="sm"
                className="text-xs sm:text-sm"
                onClick={() => {
                  logout();
                  navigate('/');
                }}
              >
                Sign Out
              </Button>
            </>
          )}
          {backendAvailable && !isAuthenticated && (
            <Button
              variant="ghost"
              size="sm"
              className="text-xs sm:text-sm"
              onClick={() => navigate('/login')}
            >
              Sign In
            </Button>
          )}
        </div>
      </div>
    </nav>
  );
}
