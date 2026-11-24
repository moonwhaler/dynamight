<script lang="ts">
  import { preferencesStore } from '../../lib/stores/preferences';
  import SettingsSection from './SettingsSection.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import * as m from '$lib/paraglide/messages.js';
</script>

<div class="space-y-6">
  <SettingsSection title={m.settings_jobs_title()} description={m.settings_jobs_desc()}>
    <div class="space-y-5">
      <Toggle
        label={m.settings_guided()}
        description={m.settings_guided_desc()}
        checked={$preferencesStore.guidedJobSetup}
        onchange={(v) => preferencesStore.setGuidedJobSetup(v)}
      />
      <Toggle
        label={m.settings_show_log_viewer()}
        description={m.settings_show_log_viewer_desc()}
        checked={$preferencesStore.showLogViewerAfterManualRun}
        onchange={(v) => preferencesStore.setShowLogViewerAfterManualRun(v)}
      />
      <Toggle
        label={m.settings_confirm_kill_process()}
        description={m.settings_confirm_kill_process_desc()}
        checked={$preferencesStore.confirmKillProcess}
        onchange={(v) => preferencesStore.setConfirmKillProcess(v)}
      />
    </div>
  </SettingsSection>

  <SettingsSection title={m.settings_log_viewer()}>
    <Toggle
      label={m.settings_auto_show_last_page()}
      description={m.settings_auto_show_last_page_desc()}
      checked={$preferencesStore.autoShowLastPage}
      onchange={(v) => preferencesStore.setAutoShowLastPage(v)}
    />
  </SettingsSection>
</div>
