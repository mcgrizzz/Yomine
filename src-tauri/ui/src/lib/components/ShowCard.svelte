<script lang="ts">
	import type { FoundEpisode, RecentFileEntry, TmdbEpisode } from '$lib/ipc';
	import { fileIcon, filename, formatOpened } from '$lib/recents';
	import {
		firstUnmined,
		knownEpisodes,
		reopenable,
		seasonLabel,
		seasonOrder,
		type Openable,
		type Show
	} from '$lib/shows';

	interface Props {
		show: Show;
		/** This show's episodes found beside opened files. */
		found: FoundEpisode[];
		/** Whether its folders are still being looked in. */
		checking: boolean;
		/** Its episodes with mined notes, as `season:episode`. */
		mined: Set<string>;
		onopen: (entry: RecentFileEntry) => void;
		onopenepisode: (episode: Openable) => void;
	}

	let { show, found, checking, mined, onopen, onopenepisode }: Props = $props();

	interface Cell {
		season: number | null;
		number: number;
		name: string | null;
		/** The reopenable ones, most recent first. */
		files: RecentFileEntry[] | undefined;
		/** Every opened one, most recent first; none for an episode that hasn't been opened. */
		opened: RecentFileEntry[] | undefined;
		/** A file of it beside opened ones, one with subtitles where there is one. */
		found: FoundEpisode | undefined;
		mined: boolean;
		state: 'opened' | 'found' | 'checking' | 'asbplayer' | 'unavailable';
	}

	const title = $derived(show.work.original_title || show.work.title);
	const english = $derived(title === show.work.title ? null : show.work.title);
	const latest = $derived(show.files[0]);

	const seasons = $derived(
		[...new Set([...show.seasons.map((s) => s.number), ...found.map((f) => f.season)])].sort(
			(a, b) => seasonOrder(a) - seasonOrder(b)
		)
	);

	let known = $state<Record<number, TmdbEpisode[]>>({});
	$effect(() => {
		if (show.work.kind !== 'tv') return;
		for (const number of seasons) {
			if (number === null) continue;
			void knownEpisodes(show.work.tmdb_id, number).then((episodes) => {
				if (episodes) known[number] = episodes;
			});
		}
	});

	function cellState(
		files: RecentFileEntry[] | undefined,
		opened: RecentFileEntry[] | undefined,
		file: FoundEpisode | undefined
	) {
		if (files) return 'opened';
		if (file?.subtitles) return 'found';
		if (checking && (!file || file.subtitles === null)) return 'checking';
		if (opened) return 'asbplayer';
		return 'unavailable';
	}

	function cells(season: number | null): Cell[] {
		const episodes = show.seasons.find((s) => s.number === season)?.episodes;
		const here = found.filter((f) => f.season === season && f.episode !== null);
		const listed = season === null ? undefined : known[season];
		const names = new Map(listed?.map((e) => [e.number, e.name]));
		const numbers = new Set([
			...names.keys(),
			...(episodes?.keys() ?? []),
			...here.map((f) => f.episode as number)
		]);
		return [...numbers]
			.sort((a, b) => a - b)
			.map((number) => {
				const opened = episodes?.get(number);
				const files = reopenable(opened);
				const own = here.filter((f) => f.episode === number);
				const file = own.find((f) => f.subtitles) ?? own[0];
				return {
					season,
					number,
					name:
						names.get(number) || opened?.[0].media?.episode_title || own[0]?.episode_title || null,
					files,
					opened,
					found: file,
					mined: mined.has(`${season}:${number}`),
					state: cellState(files, opened, file)
				};
			});
	}

	const grid = $derived(seasons.map((season) => ({ season, cells: cells(season) })));

	const unmined = $derived(firstUnmined(show, found, mined));

	// The season being watched starts expanded and the others collapsed.
	const current = $derived(latest.media?.season ?? seasons[0]);
	let toggled = $state<Record<string, boolean>>({});
	const expanded = (season: number | null) => toggled[`${season}`] ?? season === current;
	const toggle = (season: number | null) => (toggled[`${season}`] = !expanded(season));

	const episodeLabel = (season: number | null, number: number) =>
		season === null ? `E${number}` : `S${season}E${number}`;

	function status(cell: Cell): string {
		const name = cell.found && filename(cell.found.path);
		switch (cell.state) {
			case 'opened':
				return `Opened ${formatOpened((cell.files as RecentFileEntry[])[0].last_opened)}`;
			case 'found':
				return `Found: ${name}`;
			case 'checking':
				return 'Checking…';
			case 'asbplayer':
				return `Opened in asbplayer ${formatOpened((cell.opened as RecentFileEntry[])[0].last_opened)}`;
			case 'unavailable':
				return cell.found ? `Found, no Japanese subtitles: ${name}` : 'Not found';
		}
	}

	let hovered = $state<Cell | null>(null);
	const detail = $derived.by(() => {
		if (!hovered) {
			const media = latest.media;
			const episode = media?.episode == null ? '' : ` ${episodeLabel(media.season, media.episode)}`;
			return `Last opened${episode} · ${formatOpened(latest.last_opened)}`;
		}
		const { season, number, name, files } = hovered;
		return [
			episodeLabel(season, number),
			name,
			status(hovered),
			hovered.mined ? 'Mined' : null,
			files && files.length > 1 ? `${files.length} files, opens the latest` : null
		]
			.filter(Boolean)
			.join(' · ');
	});

	const hover = (cell: Cell) => ({
		onmouseenter: () => (hovered = cell),
		onmouseleave: () => (hovered = null),
		onfocus: () => (hovered = cell),
		onblur: () => (hovered = null)
	});
</script>

<div class="show">
	<div class="head">
		<div class="names">
			<span class="title" lang="ja">{title}</span>
			{#if english}<span class="english">{english}</span>{/if}
		</div>
		{#if unmined}
			{@const target = unmined}
			<button
				class="next"
				title={target.files?.[0].file_path ?? target.found?.path}
				onclick={() => onopenepisode(target)}
				>First unmined: {episodeLabel(target.season, target.number)} →</button
			>
		{/if}
	</div>
	{#each grid as { season, cells } (season)}
		{@const open = expanded(season)}
		<div class="season">
			<button class="season-label" aria-expanded={open} onclick={() => toggle(season)}
				>{open ? '▾' : '▸'} {seasonLabel(season)}</button
			>
			{#if open}
				<div class="episodes">
					{#each cells as cell (cell.number)}
						{#if cell.state === 'opened' || cell.state === 'found'}
							<button
								class="episode"
								class:latest={cell.files?.includes(latest)}
								class:mined={cell.mined}
								title={cell.files?.[0].file_path ?? cell.found?.path}
								onclick={() => onopenepisode(cell)}
								{...hover(cell)}>{cell.number}</button
							>
						{:else}
							<!-- svelte-ignore a11y_no_static_element_interactions -- hover only fills the detail line. -->
							<span
								class="episode unopened"
								class:latest={cell.opened?.includes(latest)}
								class:mined={cell.mined}
								class:checking={cell.state === 'checking'}
								onmouseenter={() => (hovered = cell)}
								onmouseleave={() => (hovered = null)}>{cell.number}</span
							>
						{/if}
					{/each}
				</div>
			{:else}
				<span class="summary"
					>{cells.filter((c) => c.opened).length}/{cells.length} opened · {cells.filter(
						(c) => c.mined
					).length} mined</span
				>
			{/if}
		</div>
	{/each}
	{#each show.loose as entry (`${entry.mode}:${entry.file_path}`)}
		{#if entry.mode === 'asbplayer'}
			<span class="file asbplayer" title={entry.file_path}
				>{fileIcon(entry.file_path)}
				{entry.title.trim() || filename(entry.file_path)}
				<span class="when">· Opened in asbplayer {formatOpened(entry.last_opened)}</span></span
			>
		{:else}
			<button class="file" title={entry.file_path} onclick={() => onopen(entry)}
				>{fileIcon(entry.file_path)}
				{entry.title.trim() || filename(entry.file_path)}
				<span class="when">· {formatOpened(entry.last_opened)}</span></button
			>
		{/if}
	{/each}
	<div class="foot">
		<p class="detail">{detail}</p>
		<span class="legend" aria-hidden="true"
			><span class="key mined"></span>Mined<span class="key"></span>Can open<span
				class="key unopened"
			></span>Can't open</span
		>
	</div>
</div>

<style>
	.show {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
		padding: 0.5rem 0.6rem;
		background: color-mix(in srgb, var(--bg-raised) 55%, transparent);
		border: 1px solid color-mix(in srgb, var(--border) 35%, transparent);
		border-radius: var(--radius);
	}
	.head {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 0.6rem;
	}
	.names {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}
	.title {
		font-size: 0.95rem;
		color: var(--text);
	}
	.english {
		font-size: 0.7rem;
		color: var(--text-muted);
	}
	.next {
		flex: none;
		padding: 0.2rem 0.6rem;
		font-size: 0.75rem;
	}
	.season {
		display: flex;
		align-items: flex-start;
		gap: 0.5rem;
	}
	.season-label {
		flex: 0 0 5rem;
		padding: 0.4rem 0 0;
		text-align: left;
		font-size: 0.7rem;
		color: var(--text-muted);
		background: transparent;
		border: none;
	}
	.season-label:hover,
	.season-label:focus-visible {
		color: var(--text);
		background: transparent;
	}
	.summary {
		padding-top: 0.4rem;
		font-size: 0.7rem;
		color: var(--text-muted);
	}
	.episodes {
		display: flex;
		flex-wrap: wrap;
		gap: 0.25rem;
	}
	.episode {
		position: relative;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 2rem;
		height: 1.85rem;
		padding: 0;
		font-size: 0.75rem;
		color: var(--text);
		background: transparent;
		border: 1px solid color-mix(in srgb, var(--accent) 55%, transparent);
		border-radius: var(--radius-sm);
	}
	.episode:hover:not(.unopened),
	.episode:focus-visible {
		background: color-mix(in srgb, var(--accent) 35%, transparent);
		border-color: var(--accent);
	}
	.episode.latest {
		border-color: var(--accent);
		box-shadow: inset 0 0 0 1px var(--accent);
	}
	.file.asbplayer {
		color: var(--text-muted);
	}
	.unopened {
		color: var(--text-muted);
		background: transparent;
		border: 1px dashed var(--border);
		cursor: default;
	}
	.mined {
		background: color-mix(in srgb, var(--accent) 30%, transparent);
	}
	.checking {
		animation: checking 1.2s ease-in-out infinite;
	}
	@keyframes checking {
		50% {
			border-color: color-mix(in srgb, var(--accent) 55%, transparent);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.checking {
			animation: none;
			border-style: dotted;
		}
	}
	.file {
		padding: 0.15rem 0.4rem;
		text-align: left;
		font-size: 0.8rem;
		color: var(--text);
		background: transparent;
		border: 1px solid transparent;
		border-radius: var(--radius-sm);
	}
	button.file:hover,
	button.file:focus-visible {
		background: var(--bg-hover);
		border-color: var(--accent);
	}
	.when {
		font-size: 0.7rem;
		color: var(--text-muted);
	}
	.foot {
		display: flex;
		align-items: center;
		gap: 0.8rem;
	}
	.legend {
		display: flex;
		align-items: center;
		gap: 0.3rem;
		flex: none;
		margin-left: auto;
		font-size: 0.65rem;
		color: var(--text-muted);
	}
	.key {
		width: 0.7rem;
		height: 0.7rem;
		margin-left: 0.4rem;
		border: 1px solid color-mix(in srgb, var(--accent) 55%, transparent);
		border-radius: 2px;
	}
	.key.unopened {
		border: 1px dashed var(--border);
	}
	.detail {
		min-width: 0;
		margin: 0;
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
		font-size: 0.7rem;
		color: var(--text-muted);
	}
</style>
