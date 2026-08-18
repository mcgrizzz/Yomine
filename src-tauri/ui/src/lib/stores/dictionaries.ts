import { get, writable } from 'svelte/store';
import * as ipc from '$lib/ipc';
import { lastError } from './ui';
import { languageToolsStatus } from './status';
import { settings } from './settings';

export type DictionaryRow = ipc.DictionaryState & { hidden: boolean };

/** Commits each *changed* entry; the backend rebakes term frequencies and emits
 * `dictionaries-changed`, which re-fetches the table via the hydrate listener. */
export async function saveDictionaryStates(entries: DictionaryRow[]): Promise<boolean> {
	try {
		for (const e of entries) {
			await ipc.setDictionaryState(e.name, e.weight, e.enabled, e.hidden);
		}
		const s = get(settings);
		if (s) {
			const frequency_weights = { ...s.frequency_weights };
			for (const e of entries) {
				frequency_weights[e.name] = {
					weight: e.weight,
					enabled: e.enabled && !e.hidden,
					hidden: e.hidden
				};
			}
			settings.set({ ...s, frequency_weights });
		}
		return true;
	} catch (err) {
		lastError.set({
			title: 'Frequency Weights',
			message: 'Failed to save dictionary settings',
			detail: String(err)
		});
		return false;
	}
}

/** `null` until the first successful check. Checked at launch and via the
 * modal's manual button only — the check hits the network. */
export const recommendedDicts = writable<ipc.RecommendedDictionary[] | null>(null);

/** A failed check keeps the previous value; the modal surfaces failures inline. */
export async function refreshRecommendedDicts(): Promise<void> {
	if (get(languageToolsStatus) !== 'ready') return;
	try {
		recommendedDicts.set(await ipc.getRecommendedDictionaries());
	} catch (err) {
		console.error('[yomine] recommended-dictionaries check failed', err);
	}
}
