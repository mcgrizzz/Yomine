import { derived, get, writable } from 'svelte/store';
import * as ipc from '$lib/ipc';
import { lastError, overlay } from './ui';
import { languageToolsStatus } from './status';
import { fileResult } from './file';
import { refreshMinedState } from './mining';

export const playerStatus = writable<ipc.PlayerStatus>({
	mpv_connected: false,
	ws_clients: 0,
	mode: 'none',
	server_state: 'stopped',
	server_error: null,
	confirmed_timestamps: []
});

/** Gates the clickable timestamp seek. */
export const playerConnected = derived(
	playerStatus,
	($p) => $p.mpv_connected || $p.ws_clients > 0
);

export async function seekTimestamp(seconds: number, label: string): Promise<void> {
	try {
		await ipc.seekTimestamp(seconds, label);
	} catch (err) {
		lastError.set({ title: 'Failed to seek', message: String(err), detail: null });
	}
}

/** Returns success so the picker can close. Cue timestamps come through, so
 * seek/👁 work immediately on the loaded table. */
export async function loadFromAsbplayer(
	media: ipc.BoundMedia,
	trackNumbers: number[] | null
): Promise<boolean> {
	if (get(languageToolsStatus) !== 'ready') return false;
	try {
		overlay.set('Fetching subtitles from asbplayer…');
		const tracks = media.loaded_subtitles;
		const track =
			trackNumbers === null
				? tracks[0]
				: (tracks.find((t) => trackNumbers.includes(t.track_number)) ?? tracks[0]);
		const result = await ipc.loadAsbplayerMedia(
			media.id,
			trackNumbers,
			media.title ?? 'asbplayer video',
			track?.file_name ?? null,
			(msg) => overlay.set(msg.message)
		);
		fileResult.set(result);
		void refreshMinedState(true);
		return true;
	} catch (err) {
		lastError.set({ title: 'Failed to load from asbplayer', message: String(err), detail: null });
		return false;
	} finally {
		overlay.set(null);
	}
}
