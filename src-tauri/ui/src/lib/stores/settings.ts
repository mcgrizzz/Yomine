// The backend owns settings; this store is a local mirror synced on each save.

import { get, writable } from 'svelte/store';
import * as ipc from '$lib/ipc';
import { lastError } from './ui';
import { posEnabled } from './controls';
import { refreshMinedState } from './mining';

export const settings = writable<ipc.SettingsData | null>(null);

/** Returns false when settings haven't hydrated yet (nothing to patch against). */
async function patchSettings(patch: Partial<ipc.SettingsData>): Promise<boolean> {
	const s = get(settings);
	if (!s) return false;
	const updated = { ...s, ...patch };
	settings.set(updated);
	await ipc.saveSettings(updated);
	return true;
}

export async function toggleDarkMode(): Promise<void> {
	const s = get(settings);
	if (!s) return;
	await patchSettings({ dark_mode: !s.dark_mode });
}

export async function toggleSerifFont(): Promise<void> {
	const s = get(settings);
	if (!s) return;
	await patchSettings({ use_serif_font: !s.use_serif_font });
}

export const setFontScale = (scale: number) =>
	patchSettings({ font_scale: Math.min(1.5, Math.max(0.75, scale)) });

export const setSentenceColoring = (mode: ipc.SentenceColoring) =>
	patchSettings({ sentence_coloring: mode });

export const setAsbplayerFollowNewMedia = (on: boolean) =>
	patchSettings({ asbplayer_follow_new_media: on });
export const setAsbplayerFollowActiveTab = (on: boolean) =>
	patchSettings({ asbplayer_follow_active_tab: on });

export const setAsbplayerPollSecs = (secs: number) =>
	patchSettings({ asbplayer_poll_secs: Math.max(1, Math.round(secs) || 1) });

/** Persists the port AND restarts a running server on it. */
export async function saveWebsocketPort(port: number): Promise<boolean> {
	try {
		await ipc.setWebsocketPort(port);
		const s = get(settings);
		if (s) settings.set({ ...s, websocket_settings: { ...s.websocket_settings, port } });
		return true;
	} catch (err) {
		lastError.set({
			title: 'WebSocket Server',
			message: 'Failed to apply the new port',
			detail: String(err)
		});
		return false;
	}
}

export async function saveAnkiSettings(
	mappings: Record<string, ipc.FieldMapping>,
	interval: number,
	yomitanUrl: string
): Promise<boolean> {
	try {
		const saved = await patchSettings({
			anki_model_mappings: mappings,
			anki_interval: interval,
			yomitan_url: yomitanUrl
		});
		// Re-probe: the Yomitan URL / sentence mappings may have changed.
		if (saved) void refreshMinedState(true);
		return saved;
	} catch (err) {
		lastError.set({
			title: 'Anki Settings',
			message: 'Failed to save settings',
			detail: String(err)
		});
		return false;
	}
}

/** Saving both persists the defaults and applies them to the live table. */
export async function savePosFilters(filters: Record<string, boolean>): Promise<boolean> {
	try {
		if (!(await patchSettings({ pos_filters: { ...filters } }))) return false;
		posEnabled.set({ ...filters });
		return true;
	} catch (err) {
		lastError.set({
			title: 'POS Filters',
			message: 'Failed to save settings',
			detail: String(err)
		});
		return false;
	}
}
