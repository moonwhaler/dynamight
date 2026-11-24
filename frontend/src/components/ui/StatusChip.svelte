<script lang="ts">
  import type { JobStatus } from '$lib/utils/jobStatus';
  import * as m from '$lib/paraglide/messages.js';

  let {
    status,
    size = 'md',
    hasRun = false,
  }: {
    status: JobStatus;
    size?: 'sm' | 'md';
    /** Whether the job ran at all (tells "not run yet" from "no good copy yet") */
    hasRun?: boolean;
  } = $props();

  const look = $derived.by(() => {
    switch (status.state) {
      case 'running':
        return { cls: 'badge-info', dot: 'bg-primary-500 animate-pulse', label: m.state_running() };
      case 'failed':
        return { cls: 'badge-error', dot: 'bg-red-500', label: m.state_failed() };
      case 'overdue':
        return { cls: 'badge-warning', dot: 'bg-amber-500', label: m.state_overdue() };
      case 'never':
        return status.attention
          ? { cls: 'badge-warning', dot: 'bg-amber-500', label: m.state_never() }
          : { cls: 'badge-gray', dot: 'bg-gray-400', label: hasRun ? m.state_no_copy() : m.state_waiting() };
      case 'ok':
        return { cls: 'badge-success', dot: 'bg-green-500', label: m.state_ok() };
      case 'disabled':
        return { cls: 'badge-gray', dot: 'bg-gray-400', label: m.state_disabled() };
    }
  });
</script>

<span class="chip {look.cls} {size === 'sm' ? 'h-5 px-1.5 text-[11px]' : ''}">
  <span class="w-1.5 h-1.5 rounded-full {look.dot}" aria-hidden="true"></span>
  {look.label}
</span>
