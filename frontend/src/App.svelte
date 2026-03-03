<script lang="ts">
  import Router from 'svelte-spa-router';
  import { wrap } from 'svelte-spa-router/wrap';
  import { authStore } from './lib/stores/auth';
  import './lib/stores/theme'; // Initialize theme store early to apply persisted theme
  import Login from './routes/Login.svelte';
  import Setup from './routes/Setup.svelte';
  import Layout from './components/layout/Layout.svelte';
  import ConfirmDialog from './components/ui/ConfirmDialog.svelte';
  import Toast from './components/ui/Toast.svelte';

  // svelte-spa-router's AsyncSvelteComponent expects Svelte 4's class-based
  // ComponentType. Svelte 5 uses a different Component type, so cast via any.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const lazy = (fn: () => Promise<unknown>) => wrap({ asyncComponent: fn as any });

  const routes = {
    '/': lazy(() => import('./routes/Dashboard.svelte')),
    '/jobs': lazy(() => import('./routes/Jobs.svelte')),
    '/jobs/new': lazy(() => import('./routes/JobDetail.svelte')),
    '/jobs/:id': lazy(() => import('./routes/JobDetail.svelte')),
    '/history': lazy(() => import('./routes/History.svelte')),
    '/files': lazy(() => import('./routes/FileBrowser.svelte')),
    '/about': lazy(() => import('./routes/About.svelte')),
  };

  // Check authentication on load
  authStore.checkAuth();
</script>

{#if $authStore.isAuthenticated}
  <Layout>
    <Router {routes} />
  </Layout>
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
