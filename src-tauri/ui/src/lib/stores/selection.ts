// Multi-select for batch mining (issue #114), keyed by `termKey`. No cross-
// store imports here: this module sits inside the player→mining→selection
// import cycle, so eval-time access to another store is a TDZ crash. The
// fileResult prune subscription lives in `hydrate.ts` for that reason.

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
