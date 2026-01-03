<script lang="ts">
  import { calculatePasswordStrength, type PasswordStrength } from '../lib/password';

  let { password = '' } = $props();

  let strength: PasswordStrength = $derived(calculatePasswordStrength(password));

  const colors = {
    weak: 'bg-red-500',
    fair: 'bg-yellow-500',
    good: 'bg-blue-500',
    strong: 'bg-green-500',
  };

  const textColors = {
    weak: 'text-red-600',
    fair: 'text-yellow-600',
    good: 'text-blue-600',
    strong: 'text-green-600',
  };

  // How many segments to fill (0-4)
  let filledSegments = $derived(
    strength.level === 'weak' ? 1 :
    strength.level === 'fair' ? 2 :
    strength.level === 'good' ? 3 : 4
  );
</script>

{#if password.length > 0}
  <div class="mt-2 space-y-1.5">
    <div class="flex gap-1">
      {#each [1, 2, 3, 4] as segment}
        <div
          class="h-1 flex-1 rounded-full transition-all duration-300 {segment <= filledSegments ? colors[strength.level] : 'bg-gray-200'}"
        ></div>
      {/each}
    </div>
    <p class="text-xs {textColors[strength.level]} transition-colors duration-300">
      {strength.label}
    </p>
  </div>
{/if}
