import { writable } from 'svelte/store';
import type * as ipc from '$lib/ipc';

/** Tools lifecycle; the UI gates file actions on `ready`. */
export const languageToolsStatus = writable<ipc.LanguageToolsStatus>('loading');

export const ankiStatus = writable<ipc.AnkiStatus>({ connected: false, fetching: false });

export const knowledge = writable<ipc.KnowledgeSummary | null>(null);
