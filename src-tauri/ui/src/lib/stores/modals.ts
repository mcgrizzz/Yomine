// Open-flags only — every modal stages its own edits and hydrates them on open.

import { derived, get, writable, type Writable } from 'svelte/store';
import * as ipc from '$lib/ipc';
import { lastError } from './ui';

export const ignoreModalOpen = writable(false);
export const websocketModalOpen = writable(false);
export const ankiModalOpen = writable(false);
export const localMediaModalOpen = writable(false);
export const frequencyModalOpen = writable(false);
export const posModalOpen = writable(false);
export const analyzerModalOpen = writable(false);
export const setupModalOpen = writable(false);
export const asbplayerModalOpen = writable(false);
export const appearanceModalOpen = writable(false);
export const aboutModalOpen = writable(false);
export const textFiltersModalOpen = writable(false);
export const browseModalOpen = writable(false);
export const epubChapterModalOpen = writable(false);
export const profilesModalOpen = writable(false);
export const autoModalOpen = writable(false);
export const matchingModalOpen = writable(false);

/** The pages of the Settings window, each a settings dialog shown in it. */
export type SettingsPage = 'appearance' | 'anki' | 'local-media' | 'websocket' | 'matching';
const PAGE_OPEN: Record<SettingsPage, Writable<boolean>> = {
	appearance: appearanceModalOpen,
	anki: ankiModalOpen,
	'local-media': localMediaModalOpen,
	websocket: websocketModalOpen,
	matching: matchingModalOpen
};
const PAGES = Object.keys(PAGE_OPEN) as SettingsPage[];
const PAGE_KEY = 'yomine:settings-page';

/** The page the Settings window shows, or `null` while it's closed. */
export const settingsPage = derived(
	PAGES.map((page) => PAGE_OPEN[page]),
	(open) => PAGES.find((_, i) => open[i]) ?? null
);

let closePage: (() => void) | null = null;
/** Sets how the open page closes: its own close, which may only warn of unsaved changes.
 * Returns what undoes it. */
export function setSettingsClose(close: () => void): () => void {
	closePage = close;
	return () => {
		if (closePage === close) closePage = null;
	};
}

/** Asks the open page to close, which closes the window once it does. */
export const closeSettings = (): void => closePage?.();

/** Opens the Settings window at the page, by default the one shown last. Switching
 * from a page with unsaved changes only warns, as closing it would. */
export function openSettings(page: SettingsPage | null = null): void {
	settingsHidden.set(false);
	const next = page ?? lastPage();
	const current = get(settingsPage);
	if (current === next) return;
	if (current) {
		closePage?.();
		if (get(PAGE_OPEN[current])) return;
	}
	try {
		localStorage.setItem(PAGE_KEY, next);
	} catch {
		// Remembering the page is only a convenience.
	}
	PAGE_OPEN[next].set(true);
}

/** Whether Settings is hidden, keeping its page and edits, while the Themes window is open. */
export const settingsHidden = writable(false);

/** Opens the Themes window beside the app, hiding Settings until it closes so the app shows. */
export async function openThemes(): Promise<void> {
	settingsHidden.set(true);
	try {
		await ipc.openThemesWindow();
	} catch (err) {
		settingsHidden.set(false);
		lastError.set({
			title: 'Themes',
			message: 'Failed to open the Themes window',
			detail: String(err)
		});
	}
}

function lastPage(): SettingsPage {
	try {
		const saved = localStorage.getItem(PAGE_KEY) as SettingsPage | null;
		if (saved && saved in PAGE_OPEN) return saved;
	} catch {
		// Falls back to the first page.
	}
	return 'appearance';
}

export const openIgnoreModal = (): void => ignoreModalOpen.set(true);
export const openWebsocketModal = (): void => openSettings('websocket');
export const openAnkiModal = (): void => openSettings('anki');
/** The note type Anki Settings opens expanded, then clears. */
export const ankiFocus = writable<string | null>(null);
export const openAnkiModalAt = (noteType: string): void => {
	ankiFocus.set(noteType);
	openSettings('anki');
};
export const openLocalMediaModal = (): void => openSettings('local-media');
export const openFrequencyModal = (): void => frequencyModalOpen.set(true);
export const openPosModal = (): void => posModalOpen.set(true);
export const openAnalyzerModal = (): void => analyzerModalOpen.set(true);
export const openSetupModal = (): void => setupModalOpen.set(true);
export const openAsbplayerModal = (): void => asbplayerModalOpen.set(true);
export const openAppearanceModal = (): void => openSettings('appearance');
export const openAboutModal = (): void => aboutModalOpen.set(true);
export const openMatchingModal = (): void => openSettings('matching');
export const openTextFiltersModal = (): void => textFiltersModalOpen.set(true);
export const openBrowseModal = (): void => browseModalOpen.set(true);
export const openEpubChapterModal = (): void => epubChapterModalOpen.set(true);
export const openProfilesModal = (): void => profilesModalOpen.set(true);
export const openAutoModal = (): void => autoModalOpen.set(true);

/** The review dialog: auto mode's picks, or the selection before mining. */
export const reviewDialogOpen = writable(false);

export const batchSummaryOpen = writable(false);
export const batchHistoryOpen = writable(false);
export const openBatchHistory = (): void => batchHistoryOpen.set(true);
