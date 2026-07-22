import { derived, get, writable } from 'svelte/store';
import type * as ipc from '$lib/ipc';
import { applyControls, freqBounds, type SortDir, type SortField } from '$lib/table';
import { fileResult } from './file';
import { settings } from './settings';

export const posCatalog = writable<ipc.PosInfo[]>([]);

export const tableSearch = writable('');

export const tableSort = writable<{ field: SortField; dir: SortDir }>({
	field: 'frequency',
	dir: 'asc'
});

/** POS-key → enabled; a missing key means enabled. */
export const posEnabled = writable<Record<string, boolean>>({});

/** JLPT chip key (N5..N1, 'none') → enabled; hydrated from `jlpt_filters`. */
export const jlptEnabled = writable<Record<string, boolean>>({});

/** `lo`/`hi` are the data bounds (slider extent), `min`/`max` the selection. */
export interface FreqFilterState {
	lo: number;
	hi: number;
	min: number;
	max: number;
	includeUnknown: boolean;
}
export const freqFilter = writable<FreqFilterState | null>(null);

// New term set → new bounds; persisted narrowing re-applies, clamped into them.
// `settings` must only be dereferenced inside the callback (import cycle, cf. player.ts).
fileResult.subscribe((r) => {
	if (!r) {
		freqFilter.set(null);
		return;
	}
	const { min: lo, max: hi } = freqBounds(r.terms);
	const s = get(settings);
	const clamp = (v: number) => Math.min(Math.max(v, lo), hi);
	const min = s?.freq_filter_min != null ? clamp(s.freq_filter_min) : lo;
	const max = s?.freq_filter_max != null ? clamp(s.freq_filter_max) : hi;
	freqFilter.set({
		lo,
		hi,
		min,
		max: Math.max(max, min),
		includeUnknown: s?.freq_include_unknown ?? false
	});
});

/** The filtered + sorted term list the table renders. */
export const visibleTerms = derived(
	[fileResult, tableSearch, tableSort, posEnabled, freqFilter, jlptEnabled],
	([$file, $search, $sort, $pos, $freq, $jlpt]) =>
		$file
			? applyControls($file.terms, $file.sentences, {
					search: $search,
					sort: $sort,
					pos: $pos,
					freq: $freq,
					jlpt: $jlpt
				})
			: []
);
