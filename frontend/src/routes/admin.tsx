import { useState, useEffect, useCallback, type FormEvent } from 'react';
import { Navigate } from 'react-router-dom';
import { useAuth, type PermissionLevel } from '@/contexts/auth-context';
import { apiFetch } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card, CardContent } from '@/components/ui/card';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';

interface AdminUser {
  id: string;
  username: string;
  display_name: string;
  permission: PermissionLevel;
  created_at: string;
}

interface AppSettings {
  registration_enabled: boolean;
}

export default function AdminRoute() {
  const { user, isAuthenticated, backendAvailable } = useAuth();
  const [users, setUsers] = useState<AdminUser[]>([]);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [newUsername, setNewUsername] = useState('');
  const [newDisplayName, setNewDisplayName] = useState('');
  const [newPermission, setNewPermission] = useState<PermissionLevel>('user');
  const [createdPassword, setCreatedPassword] = useState<string | null>(null);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  if (!backendAvailable) {
    return (
      <div className="text-center py-12 text-muted-foreground">
        <p className="text-lg font-medium mb-2">Server unavailable</p>
        <p>The admin panel requires a connection to the game server.</p>
      </div>
    );
  }

  if (!isAuthenticated || user?.permission !== 'admin') {
    return <Navigate to="/" replace />;
  }

  const fetchUsers = useCallback(async () => {
    const res = await apiFetch('/api/admin/users');
    if (res.ok) setUsers(await res.json());
  }, []);

  const fetchSettings = useCallback(async () => {
    const res = await apiFetch('/api/admin/settings');
    if (res.ok) setSettings(await res.json());
  }, []);

  useEffect(() => {
    fetchUsers();
    fetchSettings();
  }, [fetchUsers, fetchSettings]);

  const handleCreateUser = async (e: FormEvent) => {
    e.preventDefault();
    setError('');
    setCreatedPassword(null);
    setLoading(true);
    try {
      const res = await apiFetch('/api/admin/users', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          username: newUsername,
          display_name: newDisplayName || undefined,
          permission: newPermission,
        }),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body?.error?.message || 'Failed to create user');
      }
      const data = await res.json();
      setCreatedPassword(data.password);
      setNewUsername('');
      setNewDisplayName('');
      setNewPermission('user');
      fetchUsers();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Error');
    } finally {
      setLoading(false);
    }
  };

  const handlePermissionChange = async (userId: string, permission: PermissionLevel) => {
    const res = await apiFetch(`/api/admin/users/${userId}/permission`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ permission }),
    });
    if (res.ok) fetchUsers();
  };

  const handleDeleteUser = async (userId: string, username: string) => {
    if (!confirm(`Delete user "${username}"? This cannot be undone.`)) return;
    const res = await apiFetch(`/api/admin/users/${userId}`, { method: 'DELETE' });
    if (res.ok) fetchUsers();
  };

  const handleToggleRegistration = async () => {
    if (!settings) return;
    const res = await apiFetch('/api/admin/settings', {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ registration_enabled: !settings.registration_enabled }),
    });
    if (res.ok) setSettings(await res.json());
  };

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">Admin Panel</h1>

      {/* Settings */}
      <Card>
        <CardContent className="pt-6">
          <h2 className="text-lg font-semibold mb-3">Settings</h2>
          {settings && (
            <div className="flex items-center gap-3">
              <span className="text-sm">Public account registration:</span>
              <Button
                variant={settings.registration_enabled ? 'default' : 'outline'}
                size="sm"
                onClick={handleToggleRegistration}
              >
                {settings.registration_enabled ? 'Enabled' : 'Disabled'}
              </Button>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Create User */}
      <Card>
        <CardContent className="pt-6">
          <h2 className="text-lg font-semibold mb-3">Create User</h2>
          <form onSubmit={handleCreateUser} className="space-y-3">
            <div className="flex gap-3 flex-wrap">
              <Input
                placeholder="Username"
                value={newUsername}
                onChange={(e) => setNewUsername(e.target.value)}
                required
                className="flex-1 min-w-[150px]"
              />
              <Input
                placeholder="Display name (optional)"
                value={newDisplayName}
                onChange={(e) => setNewDisplayName(e.target.value)}
                className="flex-1 min-w-[150px]"
              />
              <Select value={newPermission} onValueChange={(v) => setNewPermission(v as PermissionLevel)}>
                <SelectTrigger className="w-full sm:w-[140px]">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="user">User</SelectItem>
                  <SelectItem value="moderator">Moderator</SelectItem>
                  <SelectItem value="admin">Admin</SelectItem>
                </SelectContent>
              </Select>
              <Button type="submit" disabled={loading}>
                {loading ? 'Creating...' : 'Create'}
              </Button>
            </div>
            {error && <p className="text-sm text-destructive">{error}</p>}
            {createdPassword && (
              <div className="p-3 bg-accent rounded-md">
                <p className="text-sm font-medium">Account created! Random password:</p>
                <code className="text-sm font-mono select-all">{createdPassword}</code>
                <p className="text-xs text-muted-foreground mt-1">
                  Copy this now — it won't be shown again.
                </p>
              </div>
            )}
          </form>
        </CardContent>
      </Card>

      {/* User List */}
      <Card>
        <CardContent className="pt-6">
          <h2 className="text-lg font-semibold mb-3">Users</h2>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Username</TableHead>
                <TableHead>Display Name</TableHead>
                <TableHead>Permission</TableHead>
                <TableHead>Created</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {users.map((u) => (
                <TableRow key={u.id}>
                  <TableCell className="font-medium">{u.username}</TableCell>
                  <TableCell>{u.display_name}</TableCell>
                  <TableCell>
                    {u.id === user?.id ? (
                      <span className="text-sm text-muted-foreground">{u.permission}</span>
                    ) : (
                      <Select
                        value={u.permission}
                        onValueChange={(v) => handlePermissionChange(u.id, v as PermissionLevel)}
                      >
                        <SelectTrigger className="w-[120px] h-8">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="user">user</SelectItem>
                          <SelectItem value="moderator">moderator</SelectItem>
                          <SelectItem value="admin">admin</SelectItem>
                        </SelectContent>
                      </Select>
                    )}
                  </TableCell>
                  <TableCell className="text-sm text-muted-foreground">
                    {new Date(u.created_at).toLocaleDateString()}
                  </TableCell>
                  <TableCell className="text-right">
                    {u.id !== user?.id && (
                      <Button
                        variant="destructive"
                        size="sm"
                        onClick={() => handleDeleteUser(u.id, u.username)}
                      >
                        Delete
                      </Button>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </div>
  );
}
