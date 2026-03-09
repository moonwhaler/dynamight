import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { User } from '../types';

// Mock the API module
vi.mock('../api', () => ({
  api: {
    auth: {
      setupRequired: vi.fn(),
      me: vi.fn(),
      login: vi.fn(),
      logout: vi.fn(),
      setup: vi.fn(),
    },
  },
}));

describe('authStore', () => {
  beforeEach(() => {
    vi.resetModules();
    window.location.hash = '';
  });

  it('starts with loading true', async () => {
    const { authStore } = await import('./auth');
    let state = { loading: false, isAuthenticated: false, user: null as User | null };
    const unsub = authStore.subscribe((s) => { state = { loading: s.loading, isAuthenticated: s.isAuthenticated, user: s.user }; });

    expect(state.loading).toBe(true);
    expect(state.isAuthenticated).toBe(false);
    unsub();
  });

  it('checkAuth sets setupRequired when no users', async () => {
    const { api } = await import('../api');
    vi.mocked(api.auth.setupRequired).mockResolvedValue({ setup_required: true });

    const { authStore } = await import('./auth');
    let state = { setupRequired: false, loading: false };
    const unsub = authStore.subscribe((s) => { state = { setupRequired: s.setupRequired, loading: s.loading }; });

    await authStore.checkAuth();
    expect(state.setupRequired).toBe(true);
    expect(state.loading).toBe(false);
    unsub();
  });

  it('checkAuth sets authenticated user', async () => {
    const { api } = await import('../api');
    const mockUser = { id: 1, username: 'admin' } as User;
    vi.mocked(api.auth.setupRequired).mockResolvedValue({ setup_required: false });
    vi.mocked(api.auth.me).mockResolvedValue(mockUser);

    const { authStore } = await import('./auth');
    let state = { isAuthenticated: false, user: null as User | null };
    const unsub = authStore.subscribe((s) => { state = { isAuthenticated: s.isAuthenticated, user: s.user }; });

    await authStore.checkAuth();
    expect(state.isAuthenticated).toBe(true);
    expect(state.user?.username).toBe('admin');
    unsub();
  });

  it('checkAuth handles error gracefully', async () => {
    const { api } = await import('../api');
    vi.mocked(api.auth.setupRequired).mockRejectedValue(new Error('Network'));

    const { authStore } = await import('./auth');
    let state = { isAuthenticated: false, error: null as string | null };
    const unsub = authStore.subscribe((s) => { state = { isAuthenticated: s.isAuthenticated, error: s.error }; });

    await authStore.checkAuth();
    expect(state.isAuthenticated).toBe(false);
    unsub();
  });

  it('login returns success and sets user', async () => {
    const { api } = await import('../api');
    const mockUser = { id: 1, username: 'admin' } as User;
    vi.mocked(api.auth.login).mockResolvedValue({ success: true, user: mockUser });

    const { authStore } = await import('./auth');
    let state = { isAuthenticated: false, user: null as User | null };
    const unsub = authStore.subscribe((s) => { state = { isAuthenticated: s.isAuthenticated, user: s.user }; });

    const result = await authStore.login('admin', 'password');
    expect(result).toBe('success');
    expect(state.isAuthenticated).toBe(true);
    unsub();
  });

  it('login returns totp_required when 2FA needed', async () => {
    const { api } = await import('../api');
    vi.mocked(api.auth.login).mockResolvedValue({
      requires_totp: true,
      pending_session_id: 'session-123',
    });

    const { authStore } = await import('./auth');
    let state = { pendingTotpSession: null as string | null };
    const unsub = authStore.subscribe((s) => { state = { pendingTotpSession: s.pendingTotpSession }; });

    const result = await authStore.login('admin', 'password');
    expect(result).toBe('totp_required');
    expect(state.pendingTotpSession).toBe('session-123');
    unsub();
  });

  it('login returns error on failure', async () => {
    const { api } = await import('../api');
    vi.mocked(api.auth.login).mockRejectedValue(new Error('Invalid credentials'));

    const { authStore } = await import('./auth');
    let state = { error: null as string | null };
    const unsub = authStore.subscribe((s) => { state = { error: s.error }; });

    const result = await authStore.login('admin', 'wrong');
    expect(result).toBe('error');
    expect(state.error).toBe('Invalid credentials');
    unsub();
  });

  it('logout clears state', async () => {
    const { api } = await import('../api');
    vi.mocked(api.auth.logout).mockResolvedValue(undefined);

    const { authStore } = await import('./auth');
    let state = { isAuthenticated: false, user: null as User | null };
    const unsub = authStore.subscribe((s) => { state = { isAuthenticated: s.isAuthenticated, user: s.user }; });

    await authStore.logout();
    expect(state.isAuthenticated).toBe(false);
    expect(state.user).toBeNull();
    unsub();
  });

  it('clearError resets error', async () => {
    const { api } = await import('../api');
    vi.mocked(api.auth.login).mockRejectedValue(new Error('fail'));

    const { authStore } = await import('./auth');
    let state = { error: null as string | null };
    const unsub = authStore.subscribe((s) => { state = { error: s.error }; });

    await authStore.login('a', 'b');
    expect(state.error).toBe('fail');

    authStore.clearError();
    expect(state.error).toBeNull();
    unsub();
  });
});
