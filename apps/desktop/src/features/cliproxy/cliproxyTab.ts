import { useSyncExternalStore } from "react";
import { api, type DesktopSettings } from "../../shared/api";

type CliproxyTabState = {
  ready: boolean;
  visible: boolean;
  listening: boolean | null;
};

let state: CliproxyTabState = { ready: false, visible: false, listening: null };
let request = 0;
const listeners = new Set<() => void>();

function publish(next: Partial<CliproxyTabState>) {
  const merged = { ...state, ...next };
  if (merged.ready === state.ready && merged.visible === state.visible && merged.listening === state.listening) return;
  state = merged;
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useCliproxyTabVisible() {
  return useSyncExternalStore(subscribe, () => state);
}

export function loadCliproxyTabVisible() {
  const current = request;
  return api.desktopSettings().then(
    (settings) => {
      if (current === request) publish({ ready: true, visible: Boolean(settings.show_cliproxy_tab) });
    },
    () => {
      if (current === request) publish({ ready: true });
    },
  );
}

export async function writeCliproxyTabVisible(visible: boolean) {
  const settings = await api.desktopSettings();
  const next: DesktopSettings = { ...settings, show_cliproxy_tab: visible };
  await api.setDesktopSettings(next);
  request += 1;
  publish({ ready: true, visible });
}

export function noteCliproxyListening(listening: boolean) {
  publish({ listening });
}
