<script lang="ts">
	import {
		clearMediaMatch,
		getRecentFiles,
		searchMedia,
		setMediaMatch,
		type MediaCandidate,
		type MediaMatch
	} from '$lib/ipc';
	import { mediaTitle } from '$lib/media';
	import { mediaMatch, openExternal, recentFiles } from '$lib/stores';

	type Kind = 'tv' | 'movie';
	interface Picked {
		kind: Kind;
		id: number;
		title: string;
		original_title: string;
	}

	let {
		fingerprint,
		match,
		fileTitle
	}: { fingerprint: string; match: MediaMatch | null; fileTitle: string } = $props();

	const MATCHED_BY = {
		name: 'Matched by its file name',
		content: 'Matched by another file with the same subtitles',
		user: 'Matched by you',
		suggested: 'A guess from its file name'
	};

	let root: HTMLSpanElement;
	let open = $state(false);
	let picked = $state<Picked | null>(null);
	let season = $state<number | null>(null);
	let episode = $state<number | null>(null);
	let searching = $state(false);
	let kind = $state<Kind>('tv');
	let query = $state('');
	let results = $state<MediaCandidate[]>([]);
	let loading = $state(false);
	let busy = $state(false);
	let error = $state<string | null>(null);
	let searchTimer: ReturnType<typeof setTimeout> | undefined;
	let latestSearch = 0;

	const unchanged = $derived(
		!!match &&
			!!picked &&
			picked.kind === match.work.kind &&
			picked.id === match.work.tmdb_id &&
			(picked.kind === 'movie' || (season === match.season && episode === match.episode))
	);

	function tmdbUrl(p: Picked): string {
		const work = `https://www.themoviedb.org/${p.kind}/${p.id}`;
		return p.kind === 'tv' && season !== null && episode !== null
			? `${work}/season/${season}/episode/${episode}`
			: work;
	}

	function toggle() {
		if (open) return void (open = false);
		picked = match && {
			kind: match.work.kind,
			id: match.work.tmdb_id,
			title: match.work.title,
			original_title: match.work.original_title
		};
		season = match?.season ?? null;
		episode = match?.episode ?? null;
		error = null;
		open = true;
		if (!picked) startSearch();
	}

	function startSearch() {
		searching = true;
		kind = picked?.kind ?? 'tv';
		query = fileTitle.replace(/ - (S\d+E\d+|Episode [\d.]+)$/, '');
		results = [];
		search();
	}

	function search() {
		clearTimeout(searchTimer);
		const id = ++latestSearch;
		loading = !!query.trim();
		searchTimer = setTimeout(async () => {
			const text = query.trim();
			if (!text) return void (results = []);
			try {
				const found = await searchMedia(text, kind);
				if (id !== latestSearch) return;
				kind = found.kind;
				results = found.results;
				season = found.season ?? season;
				episode = found.episode ?? episode;
				error = null;
			} catch (e) {
				error = String(e);
			} finally {
				if (id === latestSearch) loading = false;
			}
		}, 350);
	}

	function pick(result: MediaCandidate) {
		picked = {
			kind,
			id: result.id,
			title: result.title,
			original_title: result.original_title
		};
		searching = false;
	}

	async function save() {
		if (!picked) return;
		const tv = picked.kind === 'tv';
		busy = true;
		try {
			const saved = await setMediaMatch(fingerprint, {
				kind: picked.kind,
				tmdb_id: picked.id,
				season: tv ? season : null,
				episode: tv ? episode : null
			});
			mediaMatch.set(saved);
			open = false;
			recentFiles.set(await getRecentFiles());
		} catch (e) {
			error = String(e);
		} finally {
			busy = false;
		}
	}

	async function notAShow() {
		busy = true;
		try {
			await clearMediaMatch(fingerprint);
			mediaMatch.set(null);
			open = false;
			recentFiles.set(await getRecentFiles());
		} catch (e) {
			error = String(e);
		} finally {
			busy = false;
		}
	}

	function focusAndSelect(node: HTMLInputElement) {
		node.focus();
		node.select();
	}

	const year = (c: MediaCandidate) => c.release_date.slice(0, 4);
</script>

<svelte:window
	onpointerdown={(event) => {
		if (open && !root.contains(event.target as Node)) open = false;
	}}
	onkeydown={(event) => {
		if (event.key !== 'Escape' || !open) return;
		if (searching && picked) searching = false;
		else open = false;
	}}
/>

<span class="anchor" bind:this={root}>
	{#if match?.matched_by === 'suggested'}
		<button
			class="tab-chip suggested"
			title={`${MATCHED_BY.suggested}: ${match.work.title}. Click to confirm or change it.`}
			aria-expanded={open}
			onclick={toggle}>{mediaTitle(match)}?</button
		>
	{:else if match}
		<button
			class="tab-chip match"
			title={`${MATCHED_BY[match.matched_by]}. Click to change it.`}
			aria-expanded={open}
			onclick={toggle}>✓ Matched</button
		>
	{:else}
		<button
			class="tab-chip unmatched"
			title="Not matched to a show or film"
			aria-expanded={open}
			onclick={toggle}>Match…</button
		>
	{/if}

	{#if open}
		<div class="panel" role="dialog" aria-label="Match to a show or film">
			{#if searching}
				<div class="row">
					<div class="kinds" role="radiogroup" aria-label="Kind">
						<button
							role="radio"
							aria-checked={kind === 'tv'}
							class:on={kind === 'tv'}
							onclick={() => ((kind = 'tv'), search())}>TV</button
						>
						<button
							role="radio"
							aria-checked={kind === 'movie'}
							class:on={kind === 'movie'}
							onclick={() => ((kind = 'movie'), search())}>Film</button
						>
					</div>
					<input
						class="search"
						type="search"
						placeholder="Title, or a TMDB link or ID"
						bind:value={query}
						oninput={search}
						use:focusAndSelect
					/>
				</div>
				{#if results.length}
					<ul class="results">
						{#each results as result (result.id)}
							<li>
								<button onclick={() => pick(result)}>
									<span lang="ja">{result.original_title || result.title}</span>
									{#if result.original_title !== result.title}
										<span class="muted">{result.title}</span>
									{/if}
									<span class="muted year">{year(result)}</span>
								</button>
							</li>
						{/each}
					</ul>
				{:else if loading}
					<p class="muted">Searching…</p>
				{:else if query.trim()}
					<p class="muted">No results.</p>
				{/if}
			{:else if picked}
				<p class="status">
					{unchanged && match ? MATCHED_BY[match.matched_by] : 'Not saved yet'}
				</p>
				<div class="picked">
					<div class="titles">
						<strong lang="ja">{picked.original_title || picked.title}</strong>
						<span class="muted">
							{#if picked.original_title && picked.original_title !== picked.title}
								{picked.title} ·
							{/if}
							<button class="link" onclick={() => picked && openExternal(tmdbUrl(picked))}
								>TMDB ↗</button
							>
						</span>
					</div>
					<button class="change" onclick={startSearch}>Change</button>
				</div>
				{#if picked.kind === 'tv'}
					<div class="numbers">
						<label>Season <input type="number" min="0" bind:value={season} /></label>
						<label>Episode <input type="number" min="1" bind:value={episode} /></label>
					</div>
					{#if unchanged && match?.episode_title}
						<p class="muted">{match.episode_title}</p>
					{/if}
				{/if}
			{/if}

			{#if error}<p class="error">{error}</p>{/if}

			<div class="actions">
				<button class="link" disabled={busy} onclick={notAShow}>Not a show or film?</button>
				{#if searching}
					{#if picked}
						<button onclick={() => (searching = false)}>Cancel</button>
					{/if}
				{:else if !(unchanged && match?.matched_by === 'user')}
					<button class="primary" disabled={busy} onclick={save}
						>{unchanged ? 'Confirm' : 'Save'}</button
					>
				{/if}
			</div>
		</div>
	{/if}
</span>

<style>
	.anchor {
		position: relative;
		display: inline-flex;
	}
	.tab-chip {
		padding: 0.05rem 0.4rem;
		font-size: 0.72rem;
		font-family: inherit;
		border-radius: var(--radius);
		white-space: nowrap;
		border: 1px solid color-mix(in srgb, currentColor 35%, transparent);
		cursor: pointer;
	}
	.tab-chip:hover {
		background: color-mix(in srgb, currentColor 20%, transparent);
		border-color: currentColor;
	}
	.tab-chip.match {
		color: var(--text-muted);
		background: transparent;
	}
	.tab-chip.suggested {
		border-style: dashed;
		color: var(--text);
		background: var(--bg-raised);
	}
	.tab-chip.unmatched {
		color: var(--text-muted);
		background: transparent;
	}
	.panel {
		position: absolute;
		top: calc(100% + 0.3rem);
		left: 0;
		z-index: var(--z-popover);
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		width: 24rem;
		padding: 0.7rem 0.75rem;
		font-size: 0.85rem;
		background: var(--bg-panel);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		box-shadow: var(--shadow-overlay);
	}
	p {
		margin: 0;
	}
	.muted {
		color: var(--text-muted);
	}
	.row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}
	.search {
		flex: 1;
		min-width: 0;
	}
	.status {
		font-size: 0.75rem;
		color: var(--text-muted);
	}
	.picked {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.45rem 0.6rem;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: var(--radius);
	}
	.titles {
		display: flex;
		flex-direction: column;
		flex: 1;
		min-width: 0;
	}
	.titles > * {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.numbers {
		display: flex;
		gap: 1rem;
	}
	.numbers label {
		display: flex;
		align-items: center;
		gap: 0.4rem;
	}
	.numbers input {
		width: 3.5rem;
	}
	.kinds {
		display: inline-flex;
		flex-shrink: 0;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		overflow: hidden;
	}
	.kinds button {
		padding: 0.25rem 0.6rem;
		border: none;
		border-radius: 0;
		background: transparent;
	}
	.kinds button.on {
		background: var(--bg-raised);
		color: var(--text);
	}
	.results {
		margin: 0;
		padding: 0;
		list-style: none;
		max-height: 12rem;
		overflow-y: auto;
	}
	.results button {
		display: flex;
		gap: 0.5rem;
		width: 100%;
		padding: 0.3rem 0.4rem;
		text-align: left;
		border: none;
		background: transparent;
	}
	.results button:hover {
		background: var(--bg-raised);
	}
	.year {
		margin-left: auto;
	}
	.link {
		padding: 0;
		border: none;
		background: transparent;
		color: var(--text-muted);
	}
	.link:hover:not(:disabled) {
		background: transparent;
		color: var(--text);
		text-decoration: underline;
	}
	.change {
		flex-shrink: 0;
		padding: 0.2rem 0.6rem;
	}
	.error {
		color: var(--danger);
	}
	.actions {
		display: flex;
		justify-content: space-between;
		align-items: center;
		min-height: 2rem;
		padding-top: 0.6rem;
		border-top: 1px solid var(--border);
	}
</style>
