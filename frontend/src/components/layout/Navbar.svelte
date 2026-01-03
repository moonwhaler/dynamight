<script lang="ts">
  import { authStore } from '../../lib/stores/auth';
  import ChangePasswordModal from '../ChangePasswordModal.svelte';

  let showPasswordModal = $state(false);

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

    <div class="flex items-center gap-3">
      <span class="text-sm text-gray-600">
        {$authStore.user?.username}
      </span>
      <button
        onclick={() => showPasswordModal = true}
        class="p-2 text-gray-500 hover:text-gray-700 hover:bg-gray-100 rounded-lg transition-colors"
        title="Change Password"
        aria-label="Change Password"
      >
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
        </svg>
      </button>
      <button
        onclick={handleLogout}
        class="btn btn-secondary text-sm"
      >
        Logout
      </button>
    </div>
  </div>
</nav>

<ChangePasswordModal bind:open={showPasswordModal} />
