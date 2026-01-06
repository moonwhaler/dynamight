<script lang="ts">
  import * as m from '$lib/paraglide/messages.js';
</script>

<div class="-m-4 sm:-m-6 min-h-[calc(100vh-4rem)] flex items-center justify-center relative overflow-hidden">
  <!-- Animated background gradient orbs -->
  <div class="absolute inset-0 overflow-hidden pointer-events-none">
    <div class="absolute -top-40 -right-40 w-96 h-96 bg-gradient-to-br from-primary-500/20 to-cyan-500/20 rounded-full blur-3xl animate-pulse"></div>
    <div class="absolute -bottom-40 -left-40 w-96 h-96 bg-gradient-to-tr from-emerald-500/15 to-primary-500/15 rounded-full blur-3xl animate-pulse" style="animation-delay: 1s;"></div>
    <div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] bg-gradient-to-r from-primary-500/5 to-cyan-500/5 rounded-full blur-3xl"></div>
  </div>

  <!-- Content card with glassmorphism effect -->
  <div class="relative z-10 max-w-lg w-full mx-4">
    <div class="backdrop-blur-xl bg-white/70 dark:bg-gray-800/70 rounded-3xl shadow-2xl border border-white/20 dark:border-gray-700/50 p-8 md:p-12">

      <!-- Logo -->
      <div class="flex justify-center mb-8">
        <div class="relative group">
          <div class="absolute -inset-4 bg-gradient-to-r from-primary-500 to-cyan-500 rounded-2xl blur-lg opacity-20 group-hover:opacity-30 transition-opacity duration-500"></div>
          <img
            src="/logo.png"
            alt="Dynamight"
            class="relative h-20 md:h-24 w-auto drop-shadow-lg transition-transform duration-300 group-hover:scale-105"
          />
        </div>
      </div>

      <!-- Tagline -->
      <p class="text-center text-gray-600 dark:text-gray-300 text-lg mb-8 leading-relaxed">
        {m.about_tagline()}
      </p>

      <!-- Divider -->
      <div class="flex items-center justify-center gap-4 mb-8">
        <div class="h-px flex-1 bg-gradient-to-r from-transparent via-gray-300 dark:via-gray-600 to-transparent"></div>
        <svg class="w-5 h-5 text-gray-400 dark:text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M4.318 6.318a4.5 4.5 0 000 6.364L12 20.364l7.682-7.682a4.5 4.5 0 00-6.364-6.364L12 7.636l-1.318-1.318a4.5 4.5 0 00-6.364 0z" />
        </svg>
        <div class="h-px flex-1 bg-gradient-to-r from-transparent via-gray-300 dark:via-gray-600 to-transparent"></div>
      </div>

      <!-- Info items -->
      <div class="space-y-4">
        <!-- License -->
        <div class="flex items-center gap-3 text-sm text-gray-600 dark:text-gray-400">
          <div class="flex-shrink-0 w-8 h-8 rounded-lg bg-gray-100 dark:bg-gray-700/50 flex items-center justify-center">
            <svg class="w-4 h-4 text-gray-500 dark:text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
            </svg>
          </div>
          <span>{m.about_license()}</span>
          <span class="font-medium text-gray-900 dark:text-white">MIT</span>
        </div>

        <!-- Author -->
        <div class="flex items-center gap-3 text-sm text-gray-600 dark:text-gray-400">
          <div class="flex-shrink-0 w-8 h-8 rounded-lg bg-gray-100 dark:bg-gray-700/50 flex items-center justify-center">
            <svg class="w-4 h-4 text-gray-500 dark:text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
            </svg>
          </div>
          <span>{m.about_author()}</span>
          <span class="font-medium text-gray-900 dark:text-white">moonwhaler</span>
        </div>
      </div>

      <!-- GitHub button -->
      <div class="mt-8 flex justify-center">
        <a
          href="https://github.com/moonwhaler/dynamight"
          target="_blank"
          rel="noopener noreferrer"
          class="group inline-flex items-center gap-3 px-6 py-3 rounded-xl bg-gray-900 dark:bg-white text-white dark:text-gray-900 font-medium hover:bg-gray-800 dark:hover:bg-gray-100 transition-all duration-200 hover:shadow-lg hover:shadow-gray-900/20 dark:hover:shadow-white/20 hover:-translate-y-0.5"
        >
          <svg class="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
            <path fill-rule="evenodd" clip-rule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" />
          </svg>
          <span>{m.about_view_on_github()}</span>
          <svg class="w-4 h-4 opacity-50 group-hover:opacity-100 group-hover:translate-x-0.5 transition-all" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
          </svg>
        </a>
      </div>

      <!-- Version badge -->
      <div class="mt-6 flex justify-center">
        <span class="inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-medium bg-primary-50 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300">
          <span class="w-1.5 h-1.5 rounded-full bg-primary-500 animate-pulse"></span>
          {m.about_made_with_love()}
        </span>
      </div>
    </div>
  </div>
</div>

<style>
  @keyframes pulse {
    0%, 100% {
      opacity: 0.15;
    }
    50% {
      opacity: 0.25;
    }
  }

  .animate-pulse {
    animation: pulse 4s cubic-bezier(0.4, 0, 0.6, 1) infinite;
  }
</style>
