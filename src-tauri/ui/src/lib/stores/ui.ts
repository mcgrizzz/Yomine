import { writable } from 'svelte/store';
import type * as ipc from '$lib/ipc';

/** Loading overlay text (`null` = hidden). Blocks the UI while shown. */
export const overlay = writable<string | null>(null);

/** Last surfaced error (banner); `null` once dismissed. */
export const lastError = writable<ipc.ErrorPayload | null>(null);

/** True while a supported file is dragged over the window. */
export const dragHovering = writable(false);

/** Transient toast (`null` = hidden) — non-blocking, unlike `overlay`. */
export const notice = writable<string | null>(null);
let noticeTimer: ReturnType<typeof setTimeout> | undefined;

export function showNotice(text: string): void {
	notice.set(text);
	clearTimeout(noticeTimer);
	noticeTimer = setTimeout(() => notice.set(null), 4000);
}
