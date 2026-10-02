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

export function formatLastOpened(iso: string): string {
	const d = new Date(iso);
	const p = (n: number) => String(n).padStart(2, '0');
	return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}
