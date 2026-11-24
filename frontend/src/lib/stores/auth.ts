import { writable, get } from 'svelte/store';
import { api, setUnauthorizedHandler } from '../api';
import type { User } from '../types';
import * as m from '$lib/paraglide/messages.js';

// Route to restore after logging in again when the session expired
let returnRoute: string | null = null;

function goToReturnRoute() {
  window.location.hash = returnRoute ?? '#/';
  returnRoute = null;
}

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
      update((s) => ({ ...s, error: null }));
      try {
        await api.auth.setup(username, password);
        // After setup, log in the user
        const response = await api.auth.login(username, password);
        if (response.user) {
          set({ user: response.user, isAuthenticated: true, setupRequired: false, loading: false, error: null, pendingTotpSession: null });
          goToReturnRoute();
        }
        return true;
      } catch (e) {
        const message = e instanceof Error ? e.message : 'Setup failed';
        update((s) => ({ ...s, error: message }));
        return false;
      }
    },

    async setupFromBackup(file: File, password: string): Promise<boolean> {
      update((s) => ({ ...s, error: null }));
      try {
        await api.auth.setupFromBackup(file, password);
        set({ user: null, isAuthenticated: false, setupRequired: false, loading: false, error: null, pendingTotpSession: null });
        window.location.hash = '#/login';
        return true;
      } catch (e) {
        const message = e instanceof Error ? e.message : 'Restore failed';
        update((s) => ({ ...s, error: message }));
        return false;
      }
    },

    async login(username: string, password: string): Promise<'success' | 'totp_required' | 'error'> {
      update((s) => ({ ...s, error: null, pendingTotpSession: null }));
      try {
        const response = await api.auth.login(username, password);

        // Check if 2FA is required
        if (response.requires_totp && response.pending_session_id) {
          update((s) => ({ ...s, pendingTotpSession: response.pending_session_id! }));
          return 'totp_required';
        }

        // Normal login (no 2FA)
        if (response.user) {
          set({ user: response.user, isAuthenticated: true, setupRequired: false, loading: false, error: null, pendingTotpSession: null });
          goToReturnRoute();
        }
        return 'success';
      } catch (e) {
        const message = e instanceof Error ? e.message : 'Login failed';
        update((s) => ({ ...s, error: message }));
        return 'error';
      }
    },

    async validateTotp(code: string): Promise<boolean> {
      const state = get({ subscribe });
      if (!state.pendingTotpSession) return false;

      update((s) => ({ ...s, error: null }));
      try {
        const response = await api.auth.totpValidate(state.pendingTotpSession, code);
        if (response.user) {
          set({ user: response.user, isAuthenticated: true, setupRequired: false, loading: false, error: null, pendingTotpSession: null });
          goToReturnRoute();
          return true;
        }
        return false;
      } catch (e) {
        const message = e instanceof Error ? e.message : 'Invalid code';
        update((s) => ({ ...s, error: message }));
        return false;
      }
    },

    async validateRecoveryCode(recoveryCode: string): Promise<{ success: boolean; codesRemaining?: number }> {
      const state = get({ subscribe });
      if (!state.pendingTotpSession) return { success: false };

      update((s) => ({ ...s, error: null }));
      try {
        const response = await api.auth.totpRecovery(state.pendingTotpSession, recoveryCode);
        if (response.user) {
          set({ user: response.user, isAuthenticated: true, setupRequired: false, loading: false, error: null, pendingTotpSession: null });
          goToReturnRoute();
          return { success: true, codesRemaining: response.codes_remaining };
        }
        return { success: false };
      } catch (e) {
        const message = e instanceof Error ? e.message : 'Invalid recovery code';
        update((s) => ({ ...s, error: message }));
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

    /** The server rejected the session (expired or revoked): show the login form again. */
    sessionExpired() {
      if (!get({ subscribe }).isAuthenticated) return;
      const hash = window.location.hash;
      returnRoute = hash && !hash.startsWith('#/login') ? hash : null;
      set({
        user: null,
        isAuthenticated: false,
        setupRequired: false,
        loading: false,
        error: m.error_session_expired(),
        pendingTotpSession: null,
      });
    },
  };
}

export const authStore = createAuthStore();

setUnauthorizedHandler(() => authStore.sessionExpired());
