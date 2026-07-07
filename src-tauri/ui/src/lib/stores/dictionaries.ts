import { get, writable } from 'svelte/store';
import * as ipc from '$lib/ipc';
import { lastError, overlay } from './ui';
import { languageToolsStatus } from './status';
import { settings } from './settings';

/** Commits each *changed* entry; the backend rebakes term frequencies and emits
 * `dictionaries-changed`, which re-fetches the table via the hydrate listener. */
export async function saveDictionaryStates(entries: ipc.DictionaryState[]): Promise<boolean> {
	try {
		for (const e of entries) {
			await ipc.setDictionaryState(e.name, e.weight, e.enabled);
		}
		const s = get(settings);
		if (s) {
			const frequency_weights = { ...s.frequency_weights };
			for (const e of entries) {
				frequency_weights[e.name] = { weight: e.weight, enabled: e.enabled };
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

/** Zip import. The overlay only appears once the reload streams progress, so
 * nothing flashes behind the native picker; a cancelled dialog is a no-op. */
export async function loadFrequencyDictionaries(): Promise<void> {
	if (get(languageToolsStatus) !== 'ready') return;
	try {
		const copied = await ipc.loadFrequencyDictionaries((msg) => overlay.set(msg.message));
		if (copied > 0) refreshRecommendedDicts();
	} catch (err) {
		lastError.set({
			title: 'Reload Error',
			message: 'Failed to reload frequency dictionaries',
			detail: String(err)
		});
	} finally {
		overlay.set(null);
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
