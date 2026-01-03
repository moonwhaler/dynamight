<script lang="ts">
  import { authStore } from '../../lib/stores/auth';

  async function handleLogout() {
    await authStore.logout();
  }
</script>

<nav class="bg-white border-b border-gray-200 px-6 py-3">
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-3">
      <div class="w-8 h-8 bg-primary-600 rounded-lg flex items-center justify-center">
        <svg
          class="w-5 h-5 text-white"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4"
          />
        </svg>
      </div>
      <h1 class="text-xl font-bold text-gray-900">Dynamight</h1>
    </div>

    <div class="flex items-center gap-4">
      <span class="text-sm text-gray-600">
        {$authStore.user?.username}
      </span>
      <button
        onclick={handleLogout}
        class="btn btn-secondary text-sm"
      >
        Logout
      </button>
    </div>
  </div>
</nav>
