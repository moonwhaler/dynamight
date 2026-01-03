<script lang="ts">
  let { text }: { text: string } = $props();

  let isOpen = $state(false);
  let tooltipEl: HTMLDivElement | null = $state(null);
  let buttonEl: HTMLButtonElement | null = $state(null);

  function toggle(e: Event) {
    e.preventDefault();
    e.stopPropagation();
    isOpen = !isOpen;
  }

  function close() {
    isOpen = false;
  }

  // Close on click outside
  function handleClickOutside(e: MouseEvent) {
    if (isOpen && tooltipEl && buttonEl &&
        !tooltipEl.contains(e.target as Node) &&
        !buttonEl.contains(e.target as Node)) {
      close();
    }
  }

  // Close on escape
  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && isOpen) {
      close();
    }
  }
</script>

<svelte:window onclick={handleClickOutside} onkeydown={handleKeydown} />

<span class="relative inline-flex items-center">
  <button
    bind:this={buttonEl}
    type="button"
    onclick={toggle}
    class="help-button"
    aria-label="Show help"
    aria-expanded={isOpen}
  >
    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
        d="M8.228 9c.549-1.165 2.03-2 3.772-2 2.21 0 4 1.343 4 3 0 1.4-1.278 2.575-3.006 2.907-.542.104-.994.54-.994 1.093m0 3h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
    </svg>
  </button>

  {#if isOpen}
    <div
      bind:this={tooltipEl}
      class="tooltip"
      role="tooltip"
    >
      <div class="tooltip-arrow"></div>
      <div class="tooltip-content">
        {text}
      </div>
    </div>
  {/if}
</span>

<style>
  .help-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.25rem;
    height: 1.25rem;
    margin-left: 0.375rem;
    color: #9ca3af;
    background: transparent;
    border: none;
    border-radius: 9999px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .help-button:hover {
    color: #6366f1;
    background: #eef2ff;
  }

  .help-button:focus {
    outline: none;
    box-shadow: 0 0 0 2px #eef2ff, 0 0 0 4px #6366f1;
  }

  .tooltip {
    position: absolute;
    left: 50%;
    bottom: calc(100% + 0.5rem);
    transform: translateX(-50%);
    z-index: 50;
    min-width: 16rem;
    max-width: 20rem;
    animation: tooltip-enter 0.15s ease-out;
  }

  @keyframes tooltip-enter {
    from {
      opacity: 0;
      transform: translateX(-50%) translateY(4px);
    }
    to {
      opacity: 1;
      transform: translateX(-50%) translateY(0);
    }
  }

  .tooltip-arrow {
    position: absolute;
    left: 50%;
    bottom: -6px;
    transform: translateX(-50%);
    width: 12px;
    height: 12px;
    background: #1f2937;
    border-radius: 2px;
    transform: translateX(-50%) rotate(45deg);
  }

  .tooltip-content {
    position: relative;
    padding: 0.75rem 1rem;
    background: #1f2937;
    color: #f3f4f6;
    font-size: 0.8125rem;
    line-height: 1.5;
    border-radius: 0.5rem;
    box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.2), 0 8px 10px -6px rgba(0, 0, 0, 0.1);
  }
</style>
