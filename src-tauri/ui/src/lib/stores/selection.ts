// Multi-select for batch mining (issue #114), keyed by `termKey`.
// No module-scope store access: player→mining→selection is an import cycle
// (TDZ crash) — the fileResult prune subscription lives in hydrate.ts.

import { writable } from 'svelte/store';

export const selectedTerms = writable<Set<string>>(new Set());

/** Yomitan entry chosen via the popover's Queue button, keyed by termKey.
 * Missing key = default (first) entry. */
export const selectedEntryIndex = writable<Record<string, number>>({});

function dropEntryIndex(keys: string[]): void {
	selectedEntryIndex.update((m) => {
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
			dropEntryIndex([key]);
		} else {
			next.add(key);
		}
		return next;
	});
}

export function setSelected(keys: string[], on: boolean): void {
	if (!on) dropEntryIndex(keys);
	selectedTerms.update((s) => {
		const next = new Set(s);
		for (const key of keys) {
			if (on) next.add(key);
			else next.delete(key);
		}
		return next;
	});
}

/** Select a term for batch mining with a specific Yomitan entry. */
export function queueWithEntry(key: string, entryIndex: number): void {
	selectedTerms.update((s) => new Set(s).add(key));
	selectedEntryIndex.update((m) => ({ ...m, [key]: entryIndex }));
}

export function clearSelection(): void {
	selectedTerms.set(new Set());
	selectedEntryIndex.set({});
}
