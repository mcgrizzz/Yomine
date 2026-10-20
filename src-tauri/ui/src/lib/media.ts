import type { Media } from './ipc';

/** `ダンジョン飯 · S1E2`, in the original title when TMDB has one. */
export function mediaTitle(media: Media): string {
	const title = media.work.original_title || media.work.title;
	return media.season !== null && media.episode !== null
		? `${title} · S${media.season}E${media.episode}`
		: title;
}
