<script lang="ts">
  import Router from 'svelte-spa-router';
  import { wrap } from 'svelte-spa-router/wrap';
  import { authStore } from './lib/stores/auth';
  import './lib/stores/theme'; // Initialize theme store early to apply persisted theme
  import Login from './routes/Login.svelte';
  import Setup from './routes/Setup.svelte';
  import AppShell from './components/shell/AppShell.svelte';
  import ConfirmDialog from './components/ui/ConfirmDialog.svelte';
  import Toast from './components/ui/Toast.svelte';

  // svelte-spa-router's AsyncSvelteComponent expects Svelte 4's class-based
  // ComponentType. Svelte 5 uses a different Component type, so cast via any.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const lazy = (fn: () => Promise<unknown>) => wrap({ asyncComponent: fn as any });

  const routes = {
    '/': lazy(() => import('./routes/Overview.svelte')),
    '/jobs': lazy(() => import('./routes/Jobs.svelte')),
    '/jobs/new': lazy(() => import('./routes/JobNew.svelte')),
    '/jobs/:id': lazy(() => import('./routes/JobView.svelte')),
    '/jobs/:id/runs': lazy(() => import('./routes/JobView.svelte')),
    '/jobs/:id/settings': lazy(() => import('./routes/JobView.svelte')),
    '/activity': lazy(() => import('./routes/Activity.svelte')),
    '/files': lazy(() => import('./routes/FileBrowser.svelte')),
    '/settings': lazy(() => import('./routes/Settings.svelte')),
    '/settings/:section': lazy(() => import('./routes/Settings.svelte')),
    '/history': lazy(() => import('./routes/Redirect.svelte')),
    '/about': lazy(() => import('./routes/Redirect.svelte')),
    // Unknown routes (old bookmarks, typos) show the dashboard instead of an empty page
    '*': lazy(() => import('./routes/Overview.svelte')),
  };

  // Check authentication on load
  authStore.checkAuth();
</script>

{#if $authStore.isAuthenticated}
  <AppShell>
    <Router {routes} />
  </AppShell>
{:else if $authStore.loading}
  <div class="min-h-screen flex items-center justify-center">
    <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600"></div>
  </div>
{:else if $authStore.setupRequired}
  <Setup />
{:else}
  <Login />
{/if}

<!-- Global confirmation dialog -->
<ConfirmDialog />

<!-- Global toast notifications -->
<Toast />
