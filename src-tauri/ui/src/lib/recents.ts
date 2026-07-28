// Display helpers mirroring egui's `RecentFileEntry` formatters.

export const filename = (path: string): string => path.split(/[\\/]/).pop() ?? path;

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
