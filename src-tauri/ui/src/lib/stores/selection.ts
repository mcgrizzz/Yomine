// Multi-select for batch mining (issue #114), keyed by `termKey`.
// No module-scope store access: player→mining→selection is an import cycle
// (TDZ crash) — the fileResult prune subscription lives in hydrate.ts.

import { writable } from 'svelte/store';

export const selectedTerms = writable<Set<string>>(new Set());

export function toggleSelected(key: string): void {
	selectedTerms.update((s) => {
		const next = new Set(s);
		if (next.has(key)) next.delete(key);
		else next.add(key);
		return next;
	});
}

export function setSelected(keys: string[], on: boolean): void {
	selectedTerms.update((s) => {
		const next = new Set(s);
		for (const key of keys) {
			if (on) next.add(key);
			else next.delete(key);
		}
		return next;
	});
}

export function clearSelection(): void {
	selectedTerms.set(new Set());
}
