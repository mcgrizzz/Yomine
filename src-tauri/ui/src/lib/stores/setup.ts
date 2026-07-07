import { derived, writable } from 'svelte/store';
import * as ipc from '$lib/ipc';
import { settings } from './settings';

/** A command (not an event) because `get_setup_status` probes Anki/player live;
 * re-pulled after file load / settings save / dictionary changes. */
export const setupStatus = writable<ipc.SetupStatus | null>(null);

/** Best-effort: a failed probe keeps the previous snapshot. */
export async function refreshSetupStatus(): Promise<void> {
	try {
		setupStatus.set(await ipc.getSetupStatus());
	} catch (err) {
		console.error('[yomine] get_setup_status failed', err);
	}
}

export const showSetupBanner = derived([setupStatus, settings], ([$status, $settings]) => {
	if (!$status) return false;
	const freqDictMissing = !$status.has_frequency_dict;
	const ankiModelsMissing = !$settings || Object.keys($settings.anki_model_mappings).length === 0;
	return freqDictMissing || ankiModelsMissing;
});
