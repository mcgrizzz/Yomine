import { derived, writable } from 'svelte/store';
import type * as ipc from '$lib/ipc';
import { applyControls, freqBounds, type SortDir, type SortField } from '$lib/table';
import { fileResult } from './file';

export const posCatalog = writable<ipc.PosInfo[]>([]);

export const tableSearch = writable('');

export const tableSort = writable<{ field: SortField; dir: SortDir }>({
	field: 'frequency',
	dir: 'asc'
});

/** POS-key → enabled; a missing key means enabled. */
export const posEnabled = writable<Record<string, boolean>>({});

/** `lo`/`hi` are the data bounds (slider extent), `min`/`max` the selection. */
export interface FreqFilterState {
	lo: number;
	hi: number;
	min: number;
	max: number;
	includeUnknown: boolean;
}
export const freqFilter = writable<FreqFilterState | null>(null);

// New term set → new bounds; selection resets to the full range and
// unknown-frequency terms start hidden (the "?" toggle reveals them).
fileResult.subscribe((r) => {
	if (!r) {
		freqFilter.set(null);
		return;
	}
	const { min, max } = freqBounds(r.terms);
	freqFilter.set({ lo: min, hi: max, min, max, includeUnknown: false });
});

/** The filtered + sorted term list the table renders. */
export const visibleTerms = derived(
	[fileResult, tableSearch, tableSort, posEnabled, freqFilter],
	([$file, $search, $sort, $pos, $freq]) =>
		$file
			? applyControls($file.terms, $file.sentences, {
					search: $search,
					sort: $sort,
					pos: $pos,
					freq: $freq
				})
			: []
);
