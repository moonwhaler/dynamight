import { writable, get } from 'svelte/store';
import { api } from '../api';
import type { User } from '../types';

interface AuthState {
  user: User | null;
  isAuthenticated: boolean;
  setupRequired: boolean;
  loading: boolean;
  error: string | null;
  // 2FA pending state
  pendingTotpSession: string | null;
}

function createAuthStore() {
  const { subscribe, set, update } = writable<AuthState>({
    user: null,
    isAuthenticated: false,
    setupRequired: false,
    loading: true,
    error: null,
    pendingTotpSession: null,
  });

  return {
    subscribe,

    async checkAuth() {
      update((s) => ({ ...s, loading: true, error: null }));
      try {
        // First check if setup is required
        const setupResult = await api.auth.setupRequired();
        if (setupResult.setup_required) {
          set({ user: null, isAuthenticated: false, setupRequired: true, loading: false, error: null, pendingTotpSession: null });
          return;
        }

        // Then check if user is authenticated
        const user = await api.auth.me();
        set({ user, isAuthenticated: true, setupRequired: false, loading: false, error: null, pendingTotpSession: null });
      } catch {
        set({ user: null, isAuthenticated: false, setupRequired: false, loading: false, error: null, pendingTotpSession: null });
      }
    },

    async setup(username: string, password: string) {
      update((s) => ({ ...s, loading: true, error: null }));
      try {
        await api.auth.setup(username, password);
        // After setup, log in the user
        const response = await api.auth.login(username, password);
        if (response.user) {
          set({ user: response.user, isAuthenticated: true, setupRequired: false, loading: false, error: null, pendingTotpSession: null });
        }
        return true;
      } catch (e) {
        const message = e instanceof Error ? e.message : 'Setup failed';
        update((s) => ({ ...s, loading: false, error: message }));
        return false;
      }
    },

    async login(username: string, password: string): Promise<'success' | 'totp_required' | 'error'> {
      update((s) => ({ ...s, loading: true, error: null, pendingTotpSession: null }));
      try {
        const response = await api.auth.login(username, password);

        // Check if 2FA is required
        if (response.requires_totp && response.pending_session_id) {
          update((s) => ({
            ...s,
            loading: false,
            pendingTotpSession: response.pending_session_id!,
          }));
          return 'totp_required';
        }

        // Normal login (no 2FA)
        if (response.user) {
          set({ user: response.user, isAuthenticated: true, setupRequired: false, loading: false, error: null, pendingTotpSession: null });
        }
        return 'success';
      } catch (e) {
        const message = e instanceof Error ? e.message : 'Login failed';
        update((s) => ({ ...s, loading: false, error: message }));
        return 'error';
      }
    },

    async validateTotp(code: string): Promise<boolean> {
      const state = get({ subscribe });
      if (!state.pendingTotpSession) return false;

      update((s) => ({ ...s, loading: true, error: null }));
      try {
        const response = await api.auth.totpValidate(state.pendingTotpSession, code);
        if (response.user) {
          set({ user: response.user, isAuthenticated: true, setupRequired: false, loading: false, error: null, pendingTotpSession: null });
          return true;
        }
        return false;
      } catch (e) {
        const message = e instanceof Error ? e.message : 'Invalid code';
        update((s) => ({ ...s, loading: false, error: message }));
        return false;
      }
    },

    async validateRecoveryCode(recoveryCode: string): Promise<{ success: boolean; codesRemaining?: number }> {
      const state = get({ subscribe });
      if (!state.pendingTotpSession) return { success: false };

      update((s) => ({ ...s, loading: true, error: null }));
      try {
        const response = await api.auth.totpRecovery(state.pendingTotpSession, recoveryCode);
        if (response.user) {
          set({ user: response.user, isAuthenticated: true, setupRequired: false, loading: false, error: null, pendingTotpSession: null });
          return { success: true, codesRemaining: response.codes_remaining };
        }
        return { success: false };
      } catch (e) {
        const message = e instanceof Error ? e.message : 'Invalid recovery code';
        update((s) => ({ ...s, loading: false, error: message }));
        return { success: false };
      }
    },

    clearPendingSession() {
      update((s) => ({ ...s, pendingTotpSession: null, error: null }));
    },

    async logout() {
      try {
        await api.auth.logout();
      } finally {
        set({ user: null, isAuthenticated: false, setupRequired: false, loading: false, error: null, pendingTotpSession: null });
      }
    },

    clearError() {
      update((s) => ({ ...s, error: null }));
    },
  };
}

export const authStore = createAuthStore();
