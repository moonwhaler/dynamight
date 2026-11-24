<script lang="ts">
  import { api } from '$lib/api';
  import type { DirectoryEntry } from '$lib/types';
  import Icon from '../ui/Icon.svelte';
  import Menu, { type MenuItem } from '../ui/Menu.svelte';
  import * as m from '$lib/paraglide/messages.js';

  let {
    queue,
    index = $bindable(0),
    onClose,
  }: {
    /** Audio files of the folder, in listing order */
    queue: DirectoryEntry[];
    index?: number;
    onClose: () => void;
  } = $props();

  let audio = $state<HTMLAudioElement | null>(null);
  let paused = $state(true);
  let time = $state(0);
  let duration = $state(0);
  let shuffle = $state(false);
  let failed = $state(false);

  const current = $derived(queue[index] ?? null);
  const title = $derived(current ? current.name.replace(/\.[^.]+$/, '') : '');

  function step(delta: number) {
    if (queue.length === 0) return;
    if (shuffle && queue.length > 1) {
      let next = index;
      while (next === index) next = Math.floor(Math.random() * queue.length);
      index = next;
    } else {
      index = (index + delta + queue.length) % queue.length;
    }
  }

  function toggle() {
    if (!audio) return;
    if (audio.paused) audio.play().catch(() => (failed = true));
    else audio.pause();
  }

  function clock(seconds: number): string {
    if (!Number.isFinite(seconds)) return '0:00';
    const s = Math.floor(seconds % 60);
    return `${Math.floor(seconds / 60)}:${s.toString().padStart(2, '0')}`;
  }

  // A new track starts playing
  $effect(() => {
    if (!audio || !current) return;
    failed = false;
    audio.src = api.files.rawUrl(current.path, true);
    audio.play().catch(() => (failed = true));
  });

  // Lock screen and media keys
  $effect(() => {
    if (!('mediaSession' in navigator) || !current) return;
    navigator.mediaSession.metadata = new MediaMetadata({ title, album: current.path.split('/').slice(-2, -1)[0] ?? '' });
    navigator.mediaSession.setActionHandler('previoustrack', () => step(-1));
    navigator.mediaSession.setActionHandler('nexttrack', () => step(1));
    navigator.mediaSession.setActionHandler('play', () => audio?.play());
    navigator.mediaSession.setActionHandler('pause', () => audio?.pause());
    return () => {
      for (const action of ['previoustrack', 'nexttrack', 'play', 'pause'] as const) {
        navigator.mediaSession.setActionHandler(action, null);
      }
    };
  });

  const upNext = $derived<MenuItem[]>(
    queue.map((track, i) => ({
      label: `${i === index ? '▶ ' : ''}${track.name}`,
      onclick: () => (index = i),
    })),
  );
</script>

{#if current}
  <div
    class="fixed z-40 left-3 right-3 md:left-auto md:right-6 md:w-[26rem] bottom-[calc(4.75rem+env(safe-area-inset-bottom))] md:bottom-6
      rounded-2xl border border-gray-200 dark:border-gray-700 bg-white/95 dark:bg-gray-800/95 backdrop-blur shadow-xl"
    role="region"
    aria-label={m.filebrowser_player()}
  >
    <audio
      bind:this={audio}
      bind:paused
      bind:currentTime={time}
      bind:duration
      onended={() => step(1)}
      onerror={() => (failed = true)}
    ></audio>
    <div class="flex items-center gap-2 px-3 pt-2.5">
      <Icon name="audio" class="w-5 h-5 shrink-0 text-primary-600 dark:text-primary-400" />
      <div class="min-w-0 flex-1">
        <div class="text-sm font-medium text-gray-900 dark:text-white truncate" title={current.name}>{title}</div>
        <div class="text-xs text-gray-500 dark:text-gray-400 tabular-nums">
          {failed ? m.filebrowser_audio_unsupported() : `${clock(time)} / ${clock(duration)} · ${index + 1}/${queue.length}`}
        </div>
      </div>
      <Menu label={m.filebrowser_up_next()} items={upNext} icon="list" triggerClass="btn btn-ghost btn-icon btn-sm" />
      <button type="button" class="btn btn-ghost btn-icon btn-sm" onclick={onClose} aria-label={m.filebrowser_player_close()} title={m.filebrowser_player_close()}>
        <Icon name="x" class="w-4 h-4" />
      </button>
    </div>
    <input
      type="range"
      min="0"
      max={duration || 0}
      step="1"
      bind:value={time}
      class="w-full px-3 accent-primary-600"
      aria-label={m.filebrowser_seek()}
    />
    <div class="flex items-center justify-center gap-1 pb-2">
      <button
        type="button"
        class="btn btn-ghost btn-icon btn-sm {shuffle ? 'text-primary-600 dark:text-primary-400' : ''}"
        aria-pressed={shuffle}
        onclick={() => (shuffle = !shuffle)}
        aria-label={m.filebrowser_shuffle()}
        title={m.filebrowser_shuffle()}
      >
        <Icon name="arrows" class="w-4 h-4" />
      </button>
      <button type="button" class="btn btn-ghost btn-icon" onclick={() => step(-1)} aria-label={m.filebrowser_previous_track()} title={m.filebrowser_previous_track()}>
        <Icon name="chevron-left" class="w-5 h-5" />
      </button>
      <button type="button" class="btn btn-primary btn-icon" onclick={toggle} aria-label={paused ? m.filebrowser_play() : m.filebrowser_pause()}>
        <Icon name={paused ? 'play' : 'pause'} class="w-5 h-5" />
      </button>
      <button type="button" class="btn btn-ghost btn-icon" onclick={() => step(1)} aria-label={m.filebrowser_next_track()} title={m.filebrowser_next_track()}>
        <Icon name="chevron-right" class="w-5 h-5" />
      </button>
      <span class="w-8"></span>
    </div>
  </div>
{/if}
