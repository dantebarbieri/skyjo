import { useState, useEffect, useRef } from 'react';
import { Link } from 'react-router-dom';
import { z } from 'zod';
import { GeneticModelDataSchema } from '@/schemas';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { useDocumentTitle } from '@/hooks/use-document-title';
import type { GeneticModelData, SavedGenerationInfo } from '@/types';

const API_BASE = '/api';

export default function GeneticManageRoute() {
  useDocumentTitle('Manage Genetic Generations');

  const [model, setModel] = useState<GeneticModelData | null>(null);
  const [saved, setSaved] = useState<SavedGenerationInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    fetchAll();
  }, []);

  async function fetchAll() {
    try {
      const [modelRes, savedRes] = await Promise.all([
        fetch(`${API_BASE}/genetic/model`),
        fetch(`${API_BASE}/genetic/saved`),
      ]);
      if (!modelRes.ok) throw new Error('Server unavailable');
      setModel(await modelRes.json());
      if (savedRes.ok) setSaved(await savedRes.json());
      setError(null);
    } catch {
      setError('Could not connect to server. Generation management requires the game server.');
    }
  }

  async function saveLatest() {
    setSaving(true);
    try {
      const res = await fetch(`${API_BASE}/genetic/saved`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({}),
      });
      if (res.ok) {
        await fetchAll();
      } else {
        const data = await res.json().catch(() => null);
        setError(data?.message || `Failed to save (HTTP ${res.status})`);
      }
    } catch {
      setError('Failed to save generation');
    }
    setSaving(false);
  }

  async function confirmDelete() {
    if (!deleteTarget) return;
    try {
      const res = await fetch(`${API_BASE}/genetic/saved/${encodeURIComponent(deleteTarget)}`, {
        method: 'DELETE',
      });
      if (res.ok) await fetchAll();
    } catch {
      // ignore
    }
    setDeleteTarget(null);
  }

  async function exportGeneration(name: string) {
    try {
      const res = await fetch(`${API_BASE}/genetic/saved/${encodeURIComponent(name)}/model`);
      if (!res.ok) return;
      const data = await res.json();
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `${name.replace(/\s+/g, '_')}.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch {
      // ignore
    }
  }

  async function exportLatest() {
    if (!model) return;
    const blob = new Blob([JSON.stringify(model, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `genetic_gen_${model.generation}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }

  function handleImport() {
    fileInputRef.current?.click();
  }

  async function onImportFile(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    try {
      const text = await file.text();
      const data = JSON.parse(text);
      const parseResult = GeneticModelDataSchema.extend({
        best_fitness: z.number().optional(),
      }).safeParse(data);
      if (!parseResult.success) {
        setError('Invalid generation file: ' + parseResult.error.issues[0]?.message);
        return;
      }
      const name = file.name.replace(/\.json$/, '').replace(/_/g, ' ');
      const res = await fetch(`${API_BASE}/genetic/saved/import`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name,
          genome: data.best_genome,
          generation: data.generation,
          total_games_trained: data.total_games_trained,
          best_fitness: data.best_fitness,
          lineage_hash: data.lineage_hash,
        }),
      });
      if (!res.ok) {
        const err = await res.text();
        setError(`Import failed: ${err}`);
        return;
      }
      await fetchAll();
    } catch {
      setError('Failed to parse import file');
    }
    if (fileInputRef.current) fileInputRef.current.value = '';
  }

  const latestAlreadySaved = saved.some(
    (sg) => model && sg.generation === model.generation
  );

  if (error && !model) {
    return (
      <div className="space-y-4">
        <Breadcrumb />
        <Card>
          <CardContent className="py-8 text-center text-muted-foreground">
            <p className="text-sm">{error}</p>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <Breadcrumb />
      <h1 className="text-2xl font-bold">Manage Genetic Generations</h1>

      {error && (
        <div className="text-sm text-destructive bg-destructive/10 rounded-md px-3 py-2">
          {error}
          <Button size="sm" variant="ghost" className="ml-2 h-5 px-1 text-xs" onClick={() => setError(null)}>
            Dismiss
          </Button>
        </div>
      )}

      {/* Latest generation */}
      {model && (
        <Card>
          <CardHeader className="pb-2 pt-3 px-4">
            <CardTitle className="text-sm font-semibold">Latest Generation</CardTitle>
          </CardHeader>
          <CardContent className="px-4 pb-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2 flex-wrap">
                <span className="font-medium">Gen {model.generation}</span>
                <Badge variant="outline" className="text-xs">
                  {model.total_games_trained.toLocaleString()} games
                </Badge>
                {model.lineage_hash && (
                  <Badge variant="outline" className="text-xs font-mono">
                    {model.lineage_hash}
                  </Badge>
                )}
                {latestAlreadySaved && (
                  <Badge className="text-xs bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200">
                    Saved
                  </Badge>
                )}
              </div>
              <div className="flex gap-1">
                <Button
                  size="sm"
                  variant="outline"
                  onClick={saveLatest}
                  disabled={saving || model.generation === 0 || latestAlreadySaved}
                >
                  {latestAlreadySaved ? 'Saved' : 'Save'}
                </Button>
                <Button size="sm" variant="ghost" onClick={exportLatest}>
                  Export
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Saved generations */}
      <Card>
        <CardHeader className="pb-2 pt-3 px-4">
          <div className="flex items-center justify-between">
            <CardTitle className="text-sm font-semibold">
              Saved Generations ({saved.length})
            </CardTitle>
            <div className="flex gap-1">
              <Button size="sm" variant="ghost" onClick={handleImport}>
                Import
              </Button>
              <Button size="sm" variant="ghost" onClick={fetchAll}>
                Refresh
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent className="px-4 pb-3">
          {saved.length === 0 ? (
            <p className="text-sm text-muted-foreground py-4 text-center">
              No saved generations yet. Train the model and save a generation to compare later.
            </p>
          ) : (
            <div className="space-y-1">
              {saved.map((sg) => (
                <div
                  key={sg.name}
                  className="flex items-center justify-between text-sm py-2 px-2 rounded hover:bg-muted"
                >
                  <div className="flex items-center gap-2 min-w-0 flex-wrap">
                    <span className="font-medium">{sg.name}</span>
                    {sg.lineage_hash && (
                      <Badge variant="outline" className="text-[10px] font-mono shrink-0">
                        {sg.lineage_hash}
                      </Badge>
                    )}
                    <Badge variant="outline" className="text-[10px] shrink-0">
                      fitness: {sg.best_fitness.toFixed(1)} {sg.best_fitness >= 0 ? '✓' : ''}
                    </Badge>
                    <span className="text-muted-foreground text-[9px]">(less negative = better)</span>
                    <Badge variant="outline" className="text-[10px] shrink-0">
                      {sg.total_games_trained.toLocaleString()} games
                    </Badge>
                    <span className="text-xs text-muted-foreground shrink-0">
                      {formatTimestamp(sg.saved_at)}
                    </span>
                  </div>
                  <div className="flex gap-1 shrink-0">
                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-7 px-2 text-xs"
                      onClick={() => exportGeneration(sg.name)}
                    >
                      Export
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-7 px-2 text-xs text-destructive hover:text-destructive"
                      onClick={() => setDeleteTarget(sg.name)}
                    >
                      Delete
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <input
        ref={fileInputRef}
        type="file"
        accept=".json"
        className="hidden"
        onChange={onImportFile}
      />

      {/* Delete confirmation dialog */}
      <Dialog open={deleteTarget !== null} onOpenChange={(open) => !open && setDeleteTarget(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete saved generation?</DialogTitle>
            <DialogDescription>
              Are you sure you want to delete <strong>{deleteTarget}</strong>? This cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteTarget(null)}>
              Cancel
            </Button>
            <Button variant="destructive" onClick={confirmDelete}>
              Delete
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function formatTimestamp(ts: string): string {
  const secs = parseInt(ts);
  if (isNaN(secs)) return ts;
  return new Date(secs * 1000).toLocaleDateString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

function Breadcrumb() {
  return (
    <nav className="flex items-center gap-1 text-sm text-muted-foreground">
      <Link to="/rules" className="hover:text-foreground">Rules</Link>
      <span>/</span>
      <Link to="/rules/strategies" className="hover:text-foreground">Strategies</Link>
      <span>/</span>
      <Link to="/rules/strategies/Genetic" className="hover:text-foreground">Genetic</Link>
      <span>/</span>
      <span className="text-foreground">Manage</span>
    </nav>
  );
}
