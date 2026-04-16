import { useState, type FormEvent } from 'react';
import { toast } from 'sonner';
import { useAuth } from '@/contexts/auth-context';
import { apiFetch } from '@/lib/api';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card, CardContent } from '@/components/ui/card';

export default function SettingsRoute() {
  const { user, isAuthenticated, logout, backendAvailable } = useAuth();

  if (!backendAvailable) {
    return (
      <div className="text-center py-12 text-muted-foreground">
        <p className="text-lg font-medium mb-2">Server unavailable</p>
        <p>Account settings require a connection to the game server.</p>
      </div>
    );
  }

  if (!isAuthenticated || !user) {
    return (
      <div className="text-center py-12 text-muted-foreground">
        You must be signed in to access settings.
      </div>
    );
  }

  return (
    <div className="space-y-6 max-w-lg mx-auto">
      <h1 className="text-2xl font-bold">Account Settings</h1>

      <Card>
        <CardContent className="pt-6">
          <h2 className="text-lg font-semibold mb-1">Profile</h2>
          <p className="text-sm text-muted-foreground mb-4">
            Username: <span className="font-medium text-foreground">{user.username}</span>
            {' · '}
            Role: <span className="font-medium text-foreground">{user.permission}</span>
          </p>
          <DisplayNameForm currentName={user.display_name} />
        </CardContent>
      </Card>

      <Card>
        <CardContent className="pt-6">
          <h2 className="text-lg font-semibold mb-4">Change Password</h2>
          <PasswordForm />
        </CardContent>
      </Card>

      <Button variant="outline" className="w-full" onClick={logout}>
        Sign Out
      </Button>
    </div>
  );
}

function DisplayNameForm({ currentName }: { currentName: string }) {
  const [name, setName] = useState(currentName);
  const [message, setMessage] = useState('');
  const [loading, setLoading] = useState(false);
  const { refresh } = useAuth();

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setMessage('');
    setLoading(true);
    try {
      const res = await apiFetch('/api/users/me/display-name', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ display_name: name }),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body?.error?.message || 'Failed');
      }
      await refresh();
      setMessage('Display name updated!');
      toast.success('Display name updated!');
    } catch (err) {
      setMessage(err instanceof Error ? err.message : 'Error');
    } finally {
      setLoading(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-3">
      <div>
        <label className="text-sm font-medium" htmlFor="display-name">
          Display Name
        </label>
        <Input
          id="display-name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          maxLength={32}
          required
        />
      </div>
      {message && <p className="text-sm text-muted-foreground">{message}</p>}
      <Button type="submit" size="sm" disabled={loading || name === currentName}>
        {loading ? 'Saving...' : 'Update Name'}
      </Button>
    </form>
  );
}

function PasswordForm() {
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [message, setMessage] = useState('');
  const [isError, setIsError] = useState(false);
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setMessage('');

    if (newPassword !== confirmPassword) {
      setMessage('Passwords do not match');
      setIsError(true);
      return;
    }
    if (newPassword.length < 8) {
      setMessage('Password must be at least 8 characters');
      setIsError(true);
      return;
    }

    setLoading(true);
    try {
      const res = await apiFetch('/api/users/me/password', {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          current_password: currentPassword,
          new_password: newPassword,
          confirm_password: confirmPassword,
        }),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body?.error?.message || 'Failed');
      }
      setMessage('Password changed! You may need to sign in again.');
      toast.success('Password changed!');
      setIsError(false);
      setCurrentPassword('');
      setNewPassword('');
      setConfirmPassword('');
    } catch (err) {
      setMessage(err instanceof Error ? err.message : 'Error');
      setIsError(true);
    } finally {
      setLoading(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-3">
      <div>
        <label className="text-sm font-medium" htmlFor="current-pw">
          Current Password
        </label>
        <Input
          id="current-pw"
          type="password"
          value={currentPassword}
          onChange={(e) => setCurrentPassword(e.target.value)}
          autoComplete="current-password"
          required
        />
      </div>
      <div>
        <label className="text-sm font-medium" htmlFor="new-pw">
          New Password
        </label>
        <Input
          id="new-pw"
          type="password"
          value={newPassword}
          onChange={(e) => setNewPassword(e.target.value)}
          autoComplete="new-password"
          required
          minLength={8}
        />
      </div>
      <div>
        <label className="text-sm font-medium" htmlFor="confirm-pw">
          Confirm New Password
        </label>
        <Input
          id="confirm-pw"
          type="password"
          value={confirmPassword}
          onChange={(e) => setConfirmPassword(e.target.value)}
          autoComplete="new-password"
          required
          minLength={8}
        />
        {confirmPassword && newPassword !== confirmPassword && (
          <p className="text-sm text-destructive mt-1">Passwords do not match</p>
        )}
      </div>
      {message && (
        <p className={`text-sm ${isError ? 'text-destructive' : 'text-muted-foreground'}`}>
          {message}
        </p>
      )}
      <Button type="submit" size="sm" disabled={loading || (!!confirmPassword && newPassword !== confirmPassword)}>
        {loading ? 'Changing...' : 'Change Password'}
      </Button>
    </form>
  );
}
