import { writable } from 'svelte/store';
import { api } from '../api';
import type { User } from '../types';

interface AuthState {
  user: User | null;
  isAuthenticated: boolean;
  loading: boolean;
  error: string | null;
}

function createAuthStore() {
  const { subscribe, set, update } = writable<AuthState>({
    user: null,
    isAuthenticated: false,
    loading: true,
    error: null,
  });

  return {
    subscribe,

    async checkAuth() {
      update((s) => ({ ...s, loading: true, error: null }));
      try {
        const user = await api.auth.me();
        set({ user, isAuthenticated: true, loading: false, error: null });
      } catch {
        set({ user: null, isAuthenticated: false, loading: false, error: null });
      }
    },

    async login(username: string, password: string) {
      update((s) => ({ ...s, loading: true, error: null }));
      try {
        const response = await api.auth.login(username, password);
        set({ user: response.user, isAuthenticated: true, loading: false, error: null });
        return true;
      } catch (e) {
        const message = e instanceof Error ? e.message : 'Login failed';
        update((s) => ({ ...s, loading: false, error: message }));
        return false;
      }
    },

    async logout() {
      try {
        await api.auth.logout();
      } finally {
        set({ user: null, isAuthenticated: false, loading: false, error: null });
      }
    },

    clearError() {
      update((s) => ({ ...s, error: null }));
    },
  };
}

export const authStore = createAuthStore();
