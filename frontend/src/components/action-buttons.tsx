import { Trash2, Undo2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';

interface ActionButtonsProps {
  wantsFlip: boolean;
  onToggleFlip: () => void;
  onUndo: () => void;
  trashEnabled: boolean;
  undoEnabled: boolean;
}

export function ActionButtons({
  wantsFlip,
  onToggleFlip,
  onUndo,
  trashEnabled,
  undoEnabled,
}: ActionButtonsProps) {
  return (
    <TooltipProvider>
      <div className="flex flex-col items-center gap-2">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant={wantsFlip ? 'default' : 'outline'}
              size="icon"
              disabled={!trashEnabled}
              onClick={onToggleFlip}
              className="h-9 w-9"
              aria-label={wantsFlip ? 'Back to Place Mode' : 'Discard & Flip'}
              aria-pressed={wantsFlip}
            >
              <Trash2 className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="right">
            {wantsFlip ? 'Back to Place Mode' : 'Discard & Flip'}
          </TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="outline"
              size="icon"
              disabled={!undoEnabled}
              onClick={onUndo}
              className="h-9 w-9"
              aria-label="Undo Draw"
            >
              <Undo2 className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="right">
            Undo
          </TooltipContent>
        </Tooltip>
      </div>
    </TooltipProvider>
  );
}
