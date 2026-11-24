// Asks before leaving a page with unsaved changes.
//
// svelte-spa-router navigates on `hashchange`, which cannot be cancelled.
// This module registers its listener before the router does (it is imported
// from main.ts), so when a guarded page has changes it can put the old hash
// back before the router reads it, ask, and then continue or stay. Reloading
// or closing the tab is covered by `beforeunload`.
import { confirm } from '../components/ui/ConfirmDialog.svelte';
import * as m from '$lib/paraglide/messages.js';

type Check = () => boolean;

let check: Check | null = null;
let lastHash = typeof window !== 'undefined' ? window.location.hash : '';
let asking = false;

/** Guard the current page while `isDirty()` is true. Returns the release function. */
export function setLeaveGuard(isDirty: Check): () => void {
  check = isDirty;
  lastHash = window.location.hash;
  return () => {
    if (check === isDirty) check = null;
  };
}

export function hasUnsavedChanges(): boolean {
  return !!check?.();
}

async function askToLeave(): Promise<boolean> {
  return confirm({
    title: m.leave_confirm_title(),
    message: m.leave_confirm_message(),
    confirmText: m.leave_confirm_discard(),
    cancelText: m.leave_confirm_stay(),
    variant: 'warning',
  });
}

if (typeof window !== 'undefined') {
  window.addEventListener('hashchange', () => {
    const target = window.location.hash;
    if (!asking && check?.() && target !== lastHash) {
      // Undo before the router sees it, then ask
      history.replaceState(history.state, '', lastHash);
      asking = true;
      askToLeave().then((leave) => {
        asking = false;
        if (leave) {
          check = null;
          window.location.hash = target;
        }
      });
      return;
    }
    lastHash = target;
  });

  window.addEventListener('beforeunload', (e) => {
    if (check?.()) {
      e.preventDefault();
      e.returnValue = '';
    }
  });
}
