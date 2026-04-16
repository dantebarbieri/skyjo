import { Component, type ErrorInfo, type ReactNode } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';

interface Props {
  children: ReactNode;
  fallbackMessage?: string;
  onReset?: () => void;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

/**
 * Error boundary that catches React render errors and displays a recovery UI
 * instead of a blank screen. Logs error details for debugging.
 */
export class GameErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('[GameErrorBoundary] React render error:', error);
    console.error('[GameErrorBoundary] Component stack:', errorInfo.componentStack);
  }

  handleReset = () => {
    this.setState({ hasError: false, error: null });
    this.props.onReset?.();
  };

  render() {
    if (this.state.hasError) {
      return (
        <Card className="max-w-lg mx-auto mt-8">
          <CardContent className="pt-6 space-y-4">
            <h2 className="text-xl font-bold text-destructive text-center">
              Something went wrong
            </h2>
            <p className="text-sm text-muted-foreground text-center">
              {this.props.fallbackMessage ?? 'An unexpected error occurred during rendering.'}
            </p>
            {import.meta.env.DEV && this.state.error && (
              <pre className="text-xs bg-muted p-3 rounded-md overflow-auto max-h-40">
                {this.state.error.message}
              </pre>
            )}
            <div className="flex gap-2 justify-center">
              <Button variant="outline" onClick={this.handleReset}>
                Try Again
              </Button>
              <Button variant="default" onClick={() => window.location.reload()}>
                Reload Page
              </Button>
            </div>
          </CardContent>
        </Card>
      );
    }

    return this.props.children;
  }
}
