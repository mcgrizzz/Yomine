import {
	getSeasonEpisodes,
	type FoundEpisode,
	type Media,
	type MinedEpisode,
	type RecentFileEntry,
	type TmdbEpisode
} from './ipc';

export interface Season {
	/** `null` for episodes matched without a season. */
	number: number | null;
	/** Each opened episode's files, most recent first. */
	episodes: Map<number, RecentFileEntry[]>;
}

export interface Show {
	work: Media['work'];
	/** Most recent first. */
	files: RecentFileEntry[];
	seasons: Season[];
	/** A film's files, and a show's files matched without an episode. */
	loose: RecentFileEntry[];
}

/** The matched entries by show or film, in the order each was last opened. */
export function groupShows(entries: RecentFileEntry[]): {
	shows: Show[];
	unmatched: RecentFileEntry[];
} {
	const shows = new Map<string, Show>();
	const unmatched: RecentFileEntry[] = [];
	for (const entry of entries) {
		const media = entry.media;
		if (!media) {
			unmatched.push(entry);
			continue;
		}
		const key = `${media.work.kind}:${media.work.tmdb_id}`;
		let show = shows.get(key);
		if (!show) {
			show = { work: media.work, files: [], seasons: [], loose: [] };
			shows.set(key, show);
		}
		show.files.push(entry);
		if (media.episode === null) {
			show.loose.push(entry);
			continue;
		}
		let season = show.seasons.find((s) => s.number === media.season);
		if (!season) {
			season = { number: media.season, episodes: new Map() };
			show.seasons.push(season);
		}
		const files = season.episodes.get(media.episode);
		if (files) files.push(entry);
		else season.episodes.set(media.episode, [entry]);
	}
	for (const show of shows.values()) {
		show.seasons.sort((a, b) => seasonOrder(a.number) - seasonOrder(b.number));
	}
	return { shows: [...shows.values()], unmatched };
}

/** Specials last: TMDB numbers them season 0. */
export const seasonOrder = (season: number | null): number =>
	season === 0 ? Infinity : (season ?? -1);

export function seasonLabel(season: number | null): string {
	if (season === null) return 'Episodes';
	return season === 0 ? 'Specials' : `Season ${season}`;
}

const seasons = new Map<string, Promise<TmdbEpisode[] | null>>();

/** The season's aired episodes, asked for once a session. */
export function knownEpisodes(show: number, season: number): Promise<TmdbEpisode[] | null> {
	const key = `${show}:${season}`;
	let episodes = seasons.get(key);
	if (!episodes) {
		episodes = getSeasonEpisodes(show, season).catch(() => null);
		seasons.set(key, episodes);
	}
	return episodes;
}

/** The opened local episodes, as `[path, show]`, whose folders are looked in for others. */
export function openedEpisodes(entries: RecentFileEntry[]): [string, number][] {
	return entries.flatMap((entry) =>
		entry.mode === 'local' && entry.media?.work.kind === 'tv' && entry.media.episode !== null
			? [[entry.file_path, entry.media.work.tmdb_id] as [string, number]]
			: []
	);
}

/** The show's episodes with mined notes, or `found` ones mined before outside Yomine, as
 * `season:episode`. */
export const minedIn = (
	mined: MinedEpisode[],
	show: number,
	found: FoundEpisode[] = []
): Set<string> =>
	new Set([
		...mined.filter((m) => m.show === show).map((m) => `${m.season}:${m.episode}`),
		...found.filter((f) => f.mined_before).map((f) => `${f.season}:${f.episode}`)
	]);

/** The entries the Shows view can reopen. asbplayer's are left out until their video's URL
 * is kept, since the subtitle file alone doesn't play anything. */
export const reopenable = (files: RecentFileEntry[] | undefined): RecentFileEntry[] | undefined => {
	const local = files?.filter((entry) => entry.mode !== 'asbplayer');
	return local?.length ? local : undefined;
};

/** An episode to open: its reopenable files, most recent first, or a file found beside them. */
export interface Openable {
	season: number | null;
	number: number;
	files?: RecentFileEntry[];
	found?: FoundEpisode;
}

interface EpisodeNumber {
	season: number | null;
	number: number;
}

const byEpisode = (a: EpisodeNumber, b: EpisodeNumber): number =>
	seasonOrder(a.season) - seasonOrder(b.season) || a.number - b.number;

/** The episodes opened before or found with subtitles, in order, specials coming last. */
function openableEpisodes(show: Show, found: FoundEpisode[]): Openable[] {
	const episodes = new Map<string, Openable>();
	for (const season of show.seasons) {
		for (const [number, opened] of season.episodes) {
			const files = reopenable(opened);
			if (files)
				episodes.set(`${season.number}:${number}`, { season: season.number, number, files });
		}
	}
	for (const file of found) {
		if (!file.subtitles || file.episode === null) continue;
		const key = `${file.season}:${file.episode}`;
		const episode = episodes.get(key);
		if (episode) episode.found ??= file;
		else episodes.set(key, { season: file.season, number: file.episode, found: file });
	}
	return [...episodes.values()].sort(byEpisode);
}

/** The first episode with no mined notes that was opened before or found with subtitles. */
export function firstUnmined(
	show: Show,
	found: FoundEpisode[],
	mined: Set<string>
): Openable | null {
	return openableEpisodes(show, found).find((e) => !mined.has(`${e.season}:${e.number}`)) ?? null;
}

/** The first episode with no mined notes after the last one mined, else the first one. */
export function nextUnmined(
	show: Show,
	found: FoundEpisode[],
	mined: Set<string>
): Openable | null {
	const last = [...mined]
		.map((key): EpisodeNumber => {
			const [season, number] = key.split(':');
			return { season: season === 'null' ? null : Number(season), number: Number(number) };
		})
		.sort(byEpisode)
		.pop();
	const after =
		last &&
		openableEpisodes(show, found).find(
			(e) => byEpisode(e, last) > 0 && !mined.has(`${e.season}:${e.number}`)
		);
	return after || firstUnmined(show, found, mined);
}
