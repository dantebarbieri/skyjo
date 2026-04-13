import { NavLink } from 'react-router-dom';
import { cn } from '@/lib/utils';

const links = [
  { to: '/rules', label: 'Rules' },
  { to: '/play', label: 'Play' },
  { to: '/simulator', label: 'Simulator' },
];

export default function NavBar() {
  return (
    <nav className="border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
      <div className="mx-auto max-w-7xl px-4 flex items-center h-14 gap-6">
        <NavLink to="/" className="flex items-center gap-2 text-xl font-bold tracking-tight">
          <img src="/favicon.svg" alt="" className="h-7 w-7 rounded" aria-hidden="true" />
          Skyjo
        </NavLink>
        <div className="flex items-center gap-1">
          {links.map(({ to, label }) => (
            <NavLink
              key={to}
              to={to}
              className={({ isActive }) =>
                cn(
                  'px-3 py-1.5 rounded-md text-sm font-medium transition-colors',
                  isActive
                    ? 'bg-accent text-accent-foreground'
                    : 'text-muted-foreground hover:text-foreground hover:bg-accent/50'
                )
              }
            >
              {label}
            </NavLink>
          ))}
        </div>
      </div>
    </nav>
  );
}
