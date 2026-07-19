// Multi-select for batch mining (issue #114), keyed by `termKey`.
// No module-scope store access: player→mining→selection is an import cycle
// (TDZ crash) — the fileResult prune subscription lives in hydrate.ts.

import { writable } from 'svelte/store';

export const selectedTerms = writable<Set<string>>(new Set());

/** Entry/format chosen via the popover's Queue button, keyed by termKey.
 * Missing key = defaults (first entry, first format). */
export const queuedMineOptions = writable<
	Record<string, { entryIndex?: number; formatName?: string }>
>({});

function dropMineOptions(keys: string[]): void {
	queuedMineOptions.update((m) => {
		const next = { ...m };
		for (const key of keys) delete next[key];
		return next;
	});
}

export function toggleSelected(key: string): void {
	selectedTerms.update((s) => {
		const next = new Set(s);
		if (next.has(key)) {
			next.delete(key);
			dropMineOptions([key]);
		} else {
			next.add(key);
		}
		return next;
	});
}

export function setSelected(keys: string[], on: boolean): void {
	if (!on) dropMineOptions(keys);
	selectedTerms.update((s) => {
		const next = new Set(s);
		for (const key of keys) {
			if (on) next.add(key);
			else next.delete(key);
		}
		return next;
	});
}

/** Select a term for batch mining with a specific Yomitan entry/format. */
export function queueWithEntry(key: string, entryIndex: number, formatName?: string): void {
	selectedTerms.update((s) => new Set(s).add(key));
	queuedMineOptions.update((m) => ({ ...m, [key]: { entryIndex, formatName } }));
}

/** Change the card format of a queued term (the Details panel's selector). */
export function setQueuedFormat(key: string, formatName: string): void {
	queuedMineOptions.update((m) => ({ ...m, [key]: { ...m[key], formatName } }));
}

export function clearSelection(): void {
	selectedTerms.set(new Set());
	queuedMineOptions.set({});
}
