// Multi-select for batch mining (issue #114), keyed by `termKey`.
// No module-scope store access: player→mining→selection is an import cycle
// (TDZ crash) — the fileResult prune subscription lives in hydrate.ts.

import { writable } from 'svelte/store';

export const selectedTerms = writable<Set<string>>(new Set());

export type OccurrencePin = { occIdx: number; userChosen: boolean };

export interface QueuedMineOption {
	entryIndex?: number;
	formatName?: string;
	scanText?: string;
	occIdx?: number;
	/** Pin came from a ⏮/⏭ press rather than the search jump; gates conflict auto-swap. */
	userChosen?: boolean;
}

/** Entry/format chosen via the popover's Queue button, keyed by termKey.
 * Missing key = defaults (first entry, first format). */
export const queuedMineOptions = writable<Record<string, QueuedMineOption>>({});

function dropMineOptions(keys: string[]): void {
	queuedMineOptions.update((m) => {
		const next = { ...m };
		for (const key of keys) delete next[key];
		return next;
	});
}

export function pinOccurrence(key: string, pin: OccurrencePin): void {
	queuedMineOptions.update((m) => ({ ...m, [key]: { ...m[key], ...pin } }));
}

export function toggleSelected(key: string, pin?: OccurrencePin): void {
	let added = false;
	selectedTerms.update((s) => {
		const next = new Set(s);
		if (next.has(key)) next.delete(key);
		else {
			next.add(key);
			added = true;
		}
		return next;
	});
	if (!added) dropMineOptions([key]);
	else if (pin) pinOccurrence(key, pin);
}

export function setSelected(
	keys: string[],
	on: boolean,
	pinFor?: (key: string) => OccurrencePin
): void {
	if (!on) dropMineOptions(keys);
	const added: string[] = [];
	selectedTerms.update((s) => {
		const next = new Set(s);
		for (const key of keys) {
			if (!on) next.delete(key);
			else if (!next.has(key)) {
				next.add(key);
				added.push(key);
			}
		}
		return next;
	});
	if (pinFor && added.length)
		queuedMineOptions.update((m) => {
			const next = { ...m };
			for (const key of added) next[key] = { ...next[key], ...pinFor(key) };
			return next;
		});
}

/** Select a term for batch mining with a specific Yomitan entry/format. */
export function queueWithEntry(
	key: string,
	entryIndex: number,
	formatName?: string,
	scanText?: string,
	pin?: OccurrencePin
): void {
	selectedTerms.update((s) => new Set(s).add(key));
	queuedMineOptions.update((m) => ({ ...m, [key]: { entryIndex, formatName, scanText, ...pin } }));
}

/** Change the card format of a queued term (the Details panel's selector). */
export function setQueuedFormat(key: string, formatName: string): void {
	queuedMineOptions.update((m) => ({ ...m, [key]: { ...m[key], formatName } }));
}

export function clearSelection(): void {
	selectedTerms.set(new Set());
	queuedMineOptions.set({});
}
