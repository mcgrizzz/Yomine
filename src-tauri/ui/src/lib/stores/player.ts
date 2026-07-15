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

/** Ambient asbplayer active-tab awareness (asbplayer-context event). */
export const asbContext = writable<ipc.AsbplayerContext>({
	has_active_tab: false,
	active_title: null,
	active_has_subtitles: false,
	loaded_is_active: false,
	loaded_from_asbplayer: false
});

export async function seekTimestamp(seconds: number, label: string): Promise<void> {
	try {
		await ipc.seekTimestamp(seconds, label);
	} catch (err) {
		lastError.set({ title: 'Failed to seek', message: String(err), detail: null });
	}
}

/** Video path awaiting an mpv executable — drives the panel's "Locate mpv…" row. */
export const mpvLocatePrompt = writable<string | null>(null);

async function tryLaunchMpv(video: string): Promise<boolean> {
	try {
		const outcome = await ipc.launchMpv(video);
		mpvLocatePrompt.set(outcome === 'not_found' ? video : null);
		return outcome === 'launched';
	} catch (err) {
		lastError.set({ title: 'MPV', message: 'Failed to launch mpv', detail: String(err) });
		return false;
	}
}

/** Pick a video and launch mpv on it; returns true when mpv launched. */
export async function launchMpvVideo(): Promise<boolean> {
	const video = await ipc.openVideoDialog();
	if (!video) return false;
	return tryLaunchMpv(video);
}

/** Persist a user-located mpv executable, then retry the pending launch. */
export async function locateMpvAndRetry(): Promise<boolean> {
	const exe = await ipc.openExecutableDialog();
	if (!exe) return false;
	// Dynamic: a static './settings' import closes a module cycle that hits
	// controls.ts's top-level fileResult.subscribe before file.ts initializes.
	const { setMpvPath } = await import('./settings');
	await setMpvPath(exe);
	const video = get(mpvLocatePrompt);
	if (!video) return false;
	return tryLaunchMpv(video);
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
