<script lang="ts">
  import ColumnSelector from '../ui/ColumnSelector.svelte';
  import { tablePreferencesStore, ALL_COLUMNS, FIXED_COLUMNS, DEFAULT_VISIBLE } from '../../lib/stores/tablePreferences';
  import type { ColumnKey } from '../../lib/stores/tablePreferences';
  import * as m from '$lib/paraglide/messages.js';

  function columnLabel(col: string): string {
    switch (col as ColumnKey) {
      case 'status':      return m.history_table_status();
      case 'sources':     return m.job_sources();
      case 'destination': return m.job_destination();
      case 'last_run':    return m.job_last_run();
      case 'schedule':    return m.jobs_col_schedule();
      case 'options':     return m.job_options();
      default:            return col;
    }
  }

  function handleToggle(col: string) {
    const key = col as ColumnKey;
    tablePreferencesStore.setColumnVisibility(key, !$tablePreferencesStore.visibleColumns.includes(key));
  }
</script>

<ColumnSelector
  visibleColumns={$tablePreferencesStore.visibleColumns}
  allColumns={ALL_COLUMNS}
  fixedColumns={FIXED_COLUMNS}
  defaultVisible={DEFAULT_VISIBLE}
  {columnLabel}
  onToggle={handleToggle}
  onReset={() => tablePreferencesStore.reset()}
/>
