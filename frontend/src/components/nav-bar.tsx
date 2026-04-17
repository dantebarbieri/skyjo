import { useState } from 'react';
import { NavLink, useNavigate } from 'react-router-dom';
import { Menu, Trophy } from 'lucide-react';

import { cn } from '@/lib/utils';
import { useAuth } from '@/contexts/auth-context';
import { Button } from '@/components/ui/button';
import {
  Sheet,
  SheetClose,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from '@/components/ui/sheet';

const links = [
  { to: '/rules', label: 'Rules' },
  { to: '/play', label: 'Play' },
  { to: '/simulator', label: 'Simulator' },
  { to: '/leaderboard', label: 'Leaderboard', icon: Trophy },
];

type LinkVariant = 'desktop' | 'mobile';

function desktopLinkClass(isActive: boolean) {
  return cn(
    'px-2 py-1 sm:px-3 sm:py-1.5 rounded-md text-xs sm:text-sm font-medium transition-colors inline-flex items-center gap-1',
    isActive
      ? 'bg-accent text-accent-foreground'
      : 'text-muted-foreground hover:text-foreground hover:bg-accent/50'
  );
}

function mobileLinkClass(isActive: boolean) {
  return cn(
    'px-3 py-2 rounded-md text-sm font-medium transition-colors inline-flex items-center gap-2 w-full',
    isActive
      ? 'bg-accent text-accent-foreground'
      : 'text-foreground hover:bg-accent/50'
  );
}

function NavLinks({
  variant,
  onNavigate,
}: {
  variant: LinkVariant;
  onNavigate?: () => void;
}) {
  return (
    <>
      {links.map(({ to, label, icon: Icon }) => (
        <NavLink
          key={to}
          to={to}
          onClick={onNavigate}
          className={({ isActive }) =>
            variant === 'desktop'
              ? desktopLinkClass(isActive)
              : mobileLinkClass(isActive)
          }
        >
          {Icon && (
            <Icon
              className={
                variant === 'desktop' ? 'h-3.5 w-3.5' : 'h-4 w-4'
              }
            />
          )}
          {label}
        </NavLink>
      ))}
    </>
  );
}

export default function NavBar() {
  const { user, isAuthenticated, backendAvailable, logout } = useAuth();
  const navigate = useNavigate();
  const [mobileOpen, setMobileOpen] = useState(false);

  const closeMobile = () => setMobileOpen(false);

  const handleSignIn = () => {
    closeMobile();
    navigate('/login');
  };

  const handleSignOut = () => {
    closeMobile();
    logout();
    navigate('/');
  };

  return (
    <nav className="border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
      <div className="mx-auto max-w-7xl px-4 flex items-center h-14 gap-3 sm:gap-6">
        <NavLink to="/" className="text-lg sm:text-xl font-bold tracking-tight">
          Skyjo
        </NavLink>

        {/* Desktop links */}
        <div className="hidden sm:flex items-center gap-1">
          <NavLinks variant="desktop" />
        </div>

        {/* Desktop auth section — pushed to the right */}
        <div className="ml-auto hidden sm:flex items-center gap-2">
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
                onClick={handleSignOut}
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
              onClick={handleSignIn}
            >
              Sign In
            </Button>
          )}
        </div>

        {/* Mobile hamburger trigger */}
        <div className="ml-auto flex sm:hidden">
          <Sheet open={mobileOpen} onOpenChange={setMobileOpen}>
            <SheetTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                aria-label="Open menu"
              >
                <Menu className="h-5 w-5" />
              </Button>
            </SheetTrigger>
            <SheetContent side="right" className="w-72 p-0" aria-describedby={undefined}>
              <SheetHeader>
                <SheetTitle className="sr-only">Navigation</SheetTitle>
              </SheetHeader>
              <div className="flex flex-col gap-1 px-3 pb-3">
                <NavLinks variant="mobile" onNavigate={closeMobile} />
              </div>
              {backendAvailable && (
                <>
                  <div className="mx-3 h-px bg-border" />
                  <div className="flex flex-col gap-1 px-3 py-3">
                    {isAuthenticated && user ? (
                      <>
                        <NavLink
                          to="/settings"
                          onClick={closeMobile}
                          className={({ isActive }) => mobileLinkClass(isActive)}
                        >
                          {user.display_name}
                        </NavLink>
                        {user.permission === 'admin' && (
                          <NavLink
                            to="/admin"
                            onClick={closeMobile}
                            className={({ isActive }) =>
                              mobileLinkClass(isActive)
                            }
                          >
                            Admin
                          </NavLink>
                        )}
                        <SheetClose asChild>
                          <Button
                            variant="ghost"
                            className="justify-start"
                            onClick={handleSignOut}
                          >
                            Sign Out
                          </Button>
                        </SheetClose>
                      </>
                    ) : (
                      <SheetClose asChild>
                        <Button
                          variant="ghost"
                          className="justify-start"
                          onClick={handleSignIn}
                        >
                          Sign In
                        </Button>
                      </SheetClose>
                    )}
                  </div>
                </>
              )}
            </SheetContent>
          </Sheet>
        </div>
      </div>
    </nav>
  );
}
