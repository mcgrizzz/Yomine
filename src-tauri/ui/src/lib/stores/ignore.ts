import { get, writable } from 'svelte/store';
import * as ipc from '$lib/ipc';
import { lastError } from './ui';
import { languageToolsStatus } from './status';
import { fileResult } from './file';

/** Drives the term cell's greyed state. Updated optimistically on toggle; the
 * row itself is only dropped from the table on the next refresh. */
export const ignoredLemmas = writable<Set<string>>(new Set());

export async function refreshIgnoredLemmas(): Promise<void> {
	if (get(languageToolsStatus) !== 'ready') return;
	try {
		ignoredLemmas.set(new Set(await ipc.getIgnoreList()));
	} catch (err) {
		lastError.set({ title: 'Ignore list', message: String(err), detail: null });
	}
}

export async function toggleIgnore(lemma: string): Promise<void> {
	const ignored = get(ignoredLemmas).has(lemma);
	try {
		if (ignored) await ipc.removeFromIgnoreList(lemma);
		else await ipc.addToIgnoreList(lemma);
		ignoredLemmas.update((s) => {
			const next = new Set(s);
			if (ignored) next.delete(lemma);
			else next.add(lemma);
			return next;
		});
	} catch (err) {
		lastError.set({ title: 'Ignore list', message: String(err), detail: null });
	}
}

/** The backend re-filters; a returned file means the table updates in place. */
export async function saveIgnore(terms: string[], files: ipc.IgnoreFile[]): Promise<void> {
	const result = await ipc.saveIgnoreList(terms, files);
	if (result) fileResult.set(result);
	refreshIgnoredLemmas();
}
