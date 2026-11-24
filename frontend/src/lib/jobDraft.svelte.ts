// The editable state of one job, shared by the full editor and the guided
// setup. Knows what changed since loading, how to validate each part and
// how to turn itself into a save request.
import { api } from './api';
import type {
  CreateJobRequest,
  DestinationConfig,
  DestinationType,
  Job,
  Schedule,
  SyncOptions,
} from './types';
import { createDefaultDestination, createDefaultSyncOptions } from './types';
import * as m from '$lib/paraglide/messages.js';

export type DraftSection = 'general' | 'sources' | 'destination' | 'schedule' | 'options';

export class JobDraft {
  name = $state('');
  description = $state('');
  enabled = $state(true);
  sourceDirs = $state<string[]>([]);
  destinationType = $state<DestinationType>('local');
  destination = $state<DestinationConfig>(createDefaultDestination('local'));
  syncOptions = $state<SyncOptions>(createDefaultSyncOptions());
  credentialId = $state<number | null>(null);

  /** Schedules as they are on the server */
  savedSchedules = $state<Schedule[]>([]);
  /** Saved schedules as edited (removing or pausing one changes this list) */
  schedules = $state<Schedule[]>([]);
  /** New cron expressions, created when the job is saved */
  pendingSchedules = $state<string[]>([]);

  /** Snapshot of the form fields at load, to tell whether anything changed */
  #baseline = $state('');

  constructor() {
    this.#baseline = this.#fields();
  }

  #fields(): string {
    return JSON.stringify({
      name: this.name,
      description: this.description,
      enabled: this.enabled,
      sourceDirs: this.sourceDirs,
      destinationType: this.destinationType,
      destination: this.destination,
      syncOptions: this.syncOptions,
      credentialId: this.credentialId,
    });
  }

  /** Take over a job (and its schedules) as the unchanged state */
  load(job: Job, schedules: Schedule[]): void {
    this.name = job.name;
    this.description = job.description ?? '';
    this.enabled = job.enabled;
    this.sourceDirs = [...job.source_dirs];
    this.destinationType = job.destination_type || 'local';
    this.destination = job.destination ? structuredClone($state.snapshot(job.destination)) : createDefaultDestination(this.destinationType);
    this.syncOptions = job.sync_options ? structuredClone($state.snapshot(job.sync_options)) : createDefaultSyncOptions();
    this.credentialId = job.credential_id;
    this.setSavedSchedules(schedules);
    this.#baseline = this.#fields();
  }

  /** Start a new job from an existing one: same settings, new name, schedules as drafts */
  loadClone(job: Job, schedules: Schedule[], name: string): void {
    this.load(job, []);
    this.name = name;
    this.pendingSchedules = schedules.filter((s) => s.enabled).map((s) => s.cron_expression);
    // A clone is unsaved by definition
    this.#baseline = '';
  }

  setSavedSchedules(list: Schedule[]): void {
    this.savedSchedules = list;
    this.schedules = list.map((s) => ({ ...s }));
    this.pendingSchedules = [];
  }

  /** Switch the destination type, starting from that type's defaults */
  setDestinationType(type: DestinationType): void {
    if (this.destinationType === type && this.destination.type === type) return;
    this.destinationType = type;
    this.destination = createDefaultDestination(type);
    this.credentialId = null;
  }

  get schedulesDirty(): boolean {
    return (
      this.pendingSchedules.length > 0 ||
      this.schedules.length !== this.savedSchedules.length ||
      this.schedules.some((s) => this.savedSchedules.find((o) => o.id === s.id)?.enabled !== s.enabled)
    );
  }

  get dirty(): boolean {
    return this.#fields() !== this.#baseline || this.schedulesDirty;
  }

  /** Mark the current state as saved */
  markSaved(): void {
    this.#baseline = this.#fields();
  }

  /** All cron expressions that will be active after saving */
  get activeCrons(): string[] {
    return [...this.schedules.filter((s) => s.enabled).map((s) => s.cron_expression), ...this.pendingSchedules];
  }

  /** First problem in a section, or null when it is complete */
  validate(section: DraftSection): string | null {
    switch (section) {
      case 'general':
        return this.name.trim() ? null : m.job_validation_name_required();
      case 'sources':
        return this.sourceDirs.length > 0 ? null : m.job_validation_source_required();
      case 'destination':
        if (this.destinationType !== 'local' && !this.credentialId) return m.job_validation_credentials_required();
        if (this.destination.type === 'local' && !this.destination.mount_point.trim()) return m.job_validation_mount_required();
        return null;
      default:
        return null;
    }
  }

  /** The first section with a problem, in form order */
  firstInvalid(): { section: DraftSection; message: string } | null {
    for (const section of ['general', 'sources', 'destination'] as const) {
      const message = this.validate(section);
      if (message) return { section, message };
    }
    return null;
  }

  toRequest(): CreateJobRequest {
    const destination = $state.snapshot(this.destination) as DestinationConfig;
    const sync = $state.snapshot(this.syncOptions) as SyncOptions;
    return {
      name: this.name.trim(),
      description: this.description.trim() || undefined,
      enabled: this.enabled,
      source_dirs: [...this.sourceDirs],
      destination_type: this.destinationType,
      destination,
      sync_options: sync,
      credential_id: this.credentialId || undefined,
      // Older fields the server still reads for local destinations
      ...(destination.type === 'local'
        ? {
            mount_point: destination.mount_point,
            backup_subdir: destination.backup_subdir,
            usb_uuid: destination.usb_uuid || undefined,
            auto_mount: destination.auto_mount,
            auto_unmount: destination.auto_unmount,
            sync_deletes: sync.delete_extraneous,
            rsync_excludes: sync.exclude_patterns.length > 0 ? sync.exclude_patterns : undefined,
            checksum_mode: (sync.provider_options?.checksum_mode as boolean) || false,
            compress: (sync.provider_options?.compress as boolean) || false,
            dry_run: sync.dry_run,
            bandwidth_limit: sync.bandwidth_limit_kbps || undefined,
            verbosity: sync.verbosity,
          }
        : {}),
    };
  }

  /** Apply the schedule changes for a saved job. Returns how many failed. */
  async saveSchedules(jobId: number): Promise<number> {
    let failed = 0;
    for (const saved of this.savedSchedules) {
      const current = this.schedules.find((s) => s.id === saved.id);
      try {
        if (!current) await api.schedules.delete(saved.id);
        else if (current.enabled !== saved.enabled) await api.schedules.update(saved.id, { enabled: current.enabled });
      } catch {
        failed++;
      }
    }
    for (const cron of this.pendingSchedules) {
      try {
        await api.schedules.create(jobId, { cron_expression: cron });
      } catch {
        failed++;
      }
    }
    return failed;
  }
}

/** "Photos (clone)", "Photos (clone 2)", … not used by another job */
export function cloneName(baseName: string, existing: string[]): string {
  const clean = baseName.replace(/\s*\(clone(?:\s+\d+)?\)\s*$/, '').trim();
  const taken = new Set(existing);
  if (!taken.has(`${clean} (clone)`)) return `${clean} (clone)`;
  for (let n = 2; n < 100; n++) {
    const candidate = `${clean} (clone ${n})`;
    if (!taken.has(candidate)) return candidate;
  }
  return `${clean} (clone ${Date.now()})`;
}
