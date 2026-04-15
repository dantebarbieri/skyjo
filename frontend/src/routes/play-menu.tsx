import { Link } from 'react-router-dom';
import { Card, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Monitor, Globe } from 'lucide-react';
import { useDocumentTitle } from '@/hooks/use-document-title';

export default function PlayMenuRoute() {
  useDocumentTitle('Play — Skyjo');

  return (
    <div className="max-w-lg mx-auto space-y-6">
      <div className="text-center">
        <h1 className="text-2xl font-bold">Play Skyjo</h1>
        <p className="text-sm text-muted-foreground mt-2">
          Choose how you'd like to play.
        </p>
      </div>

      <div className="grid gap-4">
        <Link to="/play/local">
          <Card className="hover:border-primary transition-colors cursor-pointer">
            <CardContent className="pt-6 flex items-start gap-4">
              <Monitor className="h-8 w-8 text-primary shrink-0 mt-0.5" />
              <div>
                <h2 className="text-lg font-semibold">Local Play</h2>
                <p className="text-sm text-muted-foreground mt-1">
                  Play on this device — multiple humans sharing a screen, vs bots, or any mix.
                  Configure player count, strategies, and rules.
                </p>
              </div>
            </CardContent>
          </Card>
        </Link>

        <Link to="/play/online">
          <Card className="hover:border-primary transition-colors cursor-pointer">
            <CardContent className="pt-6 flex items-start gap-4">
              <Globe className="h-8 w-8 text-primary shrink-0 mt-0.5" />
              <div>
                <h2 className="text-lg font-semibold">Online Play</h2>
                <p className="text-sm text-muted-foreground mt-1">
                  Create or join a room — play with friends over the internet.
                  Supports 2–8 players with optional bots and turn timers.
                </p>
              </div>
            </CardContent>
          </Card>
        </Link>
      </div>
    </div>
  );
}
