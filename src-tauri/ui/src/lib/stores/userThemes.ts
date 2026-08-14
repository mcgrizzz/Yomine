// The backend owns the theme library; this store is a local mirror synced on each save.

import { get, writable } from 'svelte/store';
import * as ipc from '$lib/ipc';
import { lastError } from './ui';

/** `null` until loaded: an empty list would resolve a selected user theme to dracula. */
export const userThemes = writable<ipc.UserTheme[] | null>(null);

/** Returns false when the save failed; the mirror rolls back to the saved list. */
export async function saveUserThemes(themes: ipc.UserTheme[]): Promise<boolean> {
	const previous = get(userThemes);
	userThemes.set(themes);
	try {
		await ipc.saveUserThemes(themes);
		return true;
	} catch (err) {
		userThemes.set(previous);
		lastError.set({ title: 'Themes', message: 'Failed to save themes', detail: String(err) });
		return false;
	}
}
