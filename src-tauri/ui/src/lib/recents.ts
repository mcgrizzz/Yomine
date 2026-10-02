// Display helpers mirroring egui's `RecentFileEntry` formatters.

import type { MiningMode, RecentFileEntry } from './ipc';

/** A file loaded in both modes is listed once for each. */
export const recentKey = (entry: RecentFileEntry): string =>
	`${entry.mode ?? 'book'}:${entry.file_path}`;

export const modeLabel = (mode: MiningMode): string => (mode === 'local' ? 'Local' : 'asbplayer');

/** Books have no mode and are listed in both. */
export const inMode = (entry: RecentFileEntry, mode: MiningMode): boolean =>
	entry.mode === null || entry.mode === mode;

export const filename = (path: string): string => path.split(/[\\/]/).pop() ?? path;

export function fileIcon(path: string): string {
	switch (path.split('.').pop()?.toLowerCase()) {
		case 'epub':
			return '📖';
		case 'srt':
		case 'ass':
		case 'ssa':
		case 'mkv':
		case 'mp4':
		case 'avi':
		case 'webm':
		case 'mov':
		case 'm4v':
		case 'ts':
			return '🎬';
		default:
			return '📄';
	}
}

export function formatTermCount(n: number | null): string {
	if (n === null) return 'Unknown terms';
	return n === 1 ? '1 term' : `${n} terms`;
}

export function formatFileSize(bytes: number | null): string {
	if (bytes === null) return 'Unknown';
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
	return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** "Today, 1:29 PM", "Yesterday, …", "Sep 28", or "Sep 28, 2025" in an earlier year; the
 * time follows the system's 12- or 24-hour setting. */
export function formatOpened(iso: string): string {
	const d = new Date(iso);
	const day = (date: Date) => new Date(date.getFullYear(), date.getMonth(), date.getDate());
	const days = Math.round((day(new Date()).getTime() - day(d).getTime()) / 86_400_000);
	const time = d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
	if (days === 0) return `Today, ${time}`;
	if (days === 1) return `Yesterday, ${time}`;
	const sameYear = d.getFullYear() === new Date().getFullYear();
	return d.toLocaleDateString([], {
		month: 'short',
		day: 'numeric',
		year: sameYear ? undefined : 'numeric'
	});
}

/** What opening the entry does, when it isn't just opening it here. */
export function openHint(entry: RecentFileEntry, current: MiningMode): string | null {
	if (!entry.mode || entry.mode === current) return null;
	return entry.mode === 'asbplayer'
		? 'Switches to asbplayer and opens these subtitles.'
		: 'Switches to Local and opens this file.';
}
