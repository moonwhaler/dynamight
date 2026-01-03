<script lang="ts">
  import Router from 'svelte-spa-router';
  import { authStore } from './lib/stores/auth';
  import './lib/stores/theme'; // Initialize theme store early to apply persisted theme
  import Login from './routes/Login.svelte';
  import Setup from './routes/Setup.svelte';
  import Dashboard from './routes/Dashboard.svelte';
  import Jobs from './routes/Jobs.svelte';
  import JobDetail from './routes/JobDetail.svelte';
  import History from './routes/History.svelte';
  import Layout from './components/layout/Layout.svelte';

  const routes = {
    '/': Dashboard,
    '/jobs': Jobs,
    '/jobs/new': JobDetail,
    '/jobs/:id': JobDetail,
    '/history': History,
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
