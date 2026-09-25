<script lang="ts">
	import { untrack } from 'svelte';
	import { dirtyGuard } from '$lib/dirtyGuard.svelte';
	import { DEFAULT_HORIZON, frequencyPoints, pickPoints } from '$lib/autopick';
	import type { AutoMine } from '$lib/ipc';
	import Modal from './Modal.svelte';
	import { autoModalOpen, knowledge, posCatalog, setAutoMine, settings } from '$lib/stores';

	/** `AutoMine::default()` (core/settings.rs). */
	const DEFAULTS: AutoMine = {
		stop: 'count',
		limit: 10,
		min_score: 80,
		max_cards: 40,
		pos_points: {
			Noun: 30,
			SuruVerb: 30,
			AdjectivalNoun: 25,
			Adjective: 20,
			Verb: 20,
			Adverb: 0,
			ProperNoun: -10,
			Pronoun: -10
		},
		jlpt_points: { N5: 15, N4: 20, N3: 20, N2: 20, N1: 20 }
	};
	const JLPT_LEVELS = ['N5', 'N4', 'N3', 'N2', 'N1'];

	const copy = (a: AutoMine): AutoMine => ({
		stop: a.stop,
		limit: a.limit,
		min_score: a.min_score,
		max_cards: a.max_cards,
		pos_points: { ...a.pos_points },
		jlpt_points: Object.fromEntries(JLPT_LEVELS.map((l) => [l, a.jlpt_points[l] ?? 0]))
	});

	let draft = $state<AutoMine>(copy(DEFAULTS));
	let original = $state<AutoMine>(copy(DEFAULTS));
	let adding = $state('');

	$effect(() => {
		if ($autoModalOpen) untrack(hydrate);
	});

	function hydrate() {
		const saved = $settings?.auto_mine ?? DEFAULTS;
		draft = copy(saved);
		original = copy(saved);
		guard.disarm();
	}

	const dirty = $derived(JSON.stringify(draft) !== JSON.stringify(original));
	const guard = dirtyGuard(
		() => dirty,
		() => autoModalOpen.set(false)
	);
	const pointsValid = (n: number) => Number.isInteger(n) && n >= -50 && n <= 50;
	const valid = $derived(
		Number.isInteger(draft.limit) &&
			draft.limit >= 1 &&
			draft.limit <= 50 &&
			Number.isInteger(draft.min_score) &&
			(draft.max_cards === null || (Number.isInteger(draft.max_cards) && draft.max_cards >= 1)) &&
			[...Object.values(draft.pos_points), ...Object.values(draft.jlpt_points)].every(pointsValid)
	);

	const posName = (key: string) => $posCatalog.find((p) => p.key === key)?.display_name ?? key;
	const posRows = $derived(
		$posCatalog.map((p) => p.key).filter((key) => key in draft.pos_points)
	);
	const addable = $derived($posCatalog.filter((p) => !(p.key in draft.pos_points)));

	function addPos() {
		if (!adding) return;
		draft.pos_points[adding] = 0;
		adding = '';
	}

	function removePos(key: string) {
		delete draft.pos_points[key];
	}

	const horizon = $derived($knowledge?.horizon ?? DEFAULT_HORIZON);
	const examples = $derived(
		[
			{ rank: Math.round(horizon / 2), pos: 'Verb', jlpt: 'N3' },
			{ rank: horizon * 10, pos: 'Noun', jlpt: null }
		].map((e) => ({
			...e,
			label: `${posName(e.pos)}${e.jlpt ? `, ${e.jlpt}` : ''}, rank ${e.rank.toLocaleString()}`,
			freq: Math.round(frequencyPoints(e.rank, horizon)),
			total: Math.round(pickPoints(e.rank, e.pos, e.jlpt, horizon, draft))
		}))
	);

	async function save() {
		if (await setAutoMine(draft)) autoModalOpen.set(false);
	}
</script>

<Modal
	open={$autoModalOpen}
	title="Auto Mode"
	width="min(540px, 92%)"
	maxHeight="94%"
	onclose={guard.request}
	oninteract={guard.disarm}
>
	<div class="body">
		<p class="intro">
			Mines each new asbplayer video as it loads, from the terms your table filters show, and records
			audio and screenshots in the video tab.
		</p>

		<fieldset class="stop">
			<legend class="hint">Cards per video</legend>
			<label class="limit">
				<input type="radio" value="count" bind:group={draft.stop} />
				Mine the best
				<input
					type="number"
					min="1"
					max="50"
					aria-label="Cards per video"
					disabled={draft.stop !== 'count'}
					bind:value={draft.limit}
				/>
				cards
			</label>
			<label class="limit">
				<input type="radio" value="min_score" bind:group={draft.stop} />
				Mine every term scoring at least
				<input
					type="number"
					aria-label="Minimum score"
					disabled={draft.stop !== 'min_score'}
					bind:value={draft.min_score}
				/>
			</label>
			{#if draft.stop === 'min_score'}
				<div class="limit cap">
					<label for="auto-cap">Up to</label>
					<input
						id="auto-cap"
						type="number"
						min="1"
						disabled={draft.max_cards === null}
						value={draft.max_cards ?? ''}
						oninput={(e) => (draft.max_cards = e.currentTarget.valueAsNumber)}
					/>
					<span>cards</span>
					<label class="no-cap">
						<input
							type="checkbox"
							checked={draft.max_cards === null}
							onchange={(e) => (draft.max_cards = e.currentTarget.checked ? null : 40)}
						/>
						No cap
					</label>
				</div>
			{/if}
			<span class="hint">Fewer when the filters leave less.</span>
		</fieldset>

		<p class="hint">
			Score = frequency points + word type + JLPT points. Frequency points are 40 up to your
			horizon, rank {horizon.toLocaleString()}, and 20 fewer per tenfold step past it. The horizon
			follows your Anki cards and only moves outward. The highest total is mined first; negative
			points lower priority.
		</p>

		<div class="tables">
			<table>
				<thead><tr><th>Word type</th><th>Points</th><th></th></tr></thead>
				<tbody>
					{#each posRows as key (key)}
						<tr>
							<td>{posName(key)}</td>
							<td>
								<input
									type="number"
									min="-50"
									max="50"
									aria-label="{posName(key)} points"
									bind:value={draft.pos_points[key]}
								/>
							</td>
							<td>
								<button
									class="remove"
									title="Remove {posName(key)}"
									aria-label="Remove {posName(key)}"
									onclick={() => removePos(key)}>✕</button
								>
							</td>
						</tr>
					{/each}
					{#if addable.length > 0}
						<tr>
							<td colspan="3">
								<div class="add">
									<select bind:value={adding} aria-label="Word type to add">
										<option value="">Add word type…</option>
										{#each addable as p (p.key)}
											<option value={p.key}>{p.display_name}</option>
										{/each}
									</select>
									<button disabled={!adding} onclick={addPos}>Add</button>
								</div>
							</td>
						</tr>
					{/if}
				</tbody>
			</table>

			<table>
				<thead><tr><th>JLPT level</th><th>Points</th></tr></thead>
				<tbody>
					{#each JLPT_LEVELS as level (level)}
						<tr>
							<td>{level}</td>
							<td>
								<input
									type="number"
									min="-50"
									max="50"
									aria-label="{level} points"
									bind:value={draft.jlpt_points[level]}
								/>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
		{#if !valid}
			<p class="invalid">⚠ Cards per video must be 1–50, the cap at least 1, and points between -50 and 50</p>
		{/if}
	</div>

	{#snippet footer()}
		<table class="examples">
			<thead><tr><th>Example</th><th>Frequency</th><th>Total</th></tr></thead>
			<tbody>
				{#each examples as e (e.label)}
					<tr><td>{e.label}</td><td>{e.freq}</td><td>{e.total}</td></tr>
				{/each}
			</tbody>
		</table>
		<hr />
		{#if guard.armed || dirty}
			<div class="status">
				{guard.armed ? '⚠ Unsaved changes — dismiss again to discard' : '⚠ Settings have been modified'}
			</div>
		{/if}
		<footer>
			<button class="primary" disabled={!dirty || !valid} onclick={save}>Save Settings</button>
			<button disabled={!dirty} onclick={() => (draft = copy(original))}>Cancel</button>
			<button class="right" onclick={() => (draft = copy(DEFAULTS))}>Restore Default</button>
		</footer>
	{/snippet}
</Modal>

<style>
	.body {
		display: flex;
		flex-direction: column;
		gap: 0.45rem;
		min-height: 0;
		padding: 0 1rem;
	}
	p {
		margin: 0;
	}
	.intro {
		font-size: 0.9rem;
	}
	.stop {
		display: grid;
		gap: 0.35rem;
		margin: 0;
		padding: 0;
		border: none;
	}
	.stop legend {
		padding: 0;
		margin-bottom: 0.2rem;
	}
	.cap {
		padding-left: 1.6rem;
	}
	.no-cap {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
	}
	.limit {
		display: flex;
		align-items: center;
		gap: 0.6rem;
	}
	input[type='number'] {
		width: 4.5rem;
		padding: 0.2rem 0.45rem;
		background: var(--bg-raised);
		color: var(--text);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
	}
	.hint {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.invalid {
		font-size: 0.85rem;
		color: var(--danger);
	}
	/* The one scroll region: the local backgrounds cover the shadows at either end, so a
	   shadow only shows where more rows are hidden. */
	.tables {
		display: grid;
		grid-template-columns: 3fr 2fr;
		gap: 1rem;
		align-items: start;
		flex: 1 1 auto;
		min-height: 9rem;
		overflow-y: auto;
		border-bottom: 1px solid var(--border);
		background:
			linear-gradient(var(--bg-panel) 30%, transparent) top,
			linear-gradient(transparent, var(--bg-panel) 70%) bottom,
			radial-gradient(
					farthest-side at 50% 0,
					color-mix(in srgb, var(--text) 20%, transparent),
					transparent
				)
				top,
			radial-gradient(
					farthest-side at 50% 100%,
					color-mix(in srgb, var(--text) 20%, transparent),
					transparent
				)
				bottom;
		background-repeat: no-repeat;
		background-size:
			100% 1.5rem,
			100% 1.5rem,
			100% 0.6rem,
			100% 0.6rem;
		background-attachment: local, local, scroll, scroll;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.85rem;
	}
	th {
		text-align: left;
		font-weight: normal;
		color: var(--text-muted);
	}
	.tables th {
		position: sticky;
		top: 0;
		z-index: 1;
		background: var(--bg-panel);
		box-shadow: inset 0 -1px var(--border);
	}
	td,
	th {
		padding: 0.18rem 0.3rem;
	}
	.remove {
		padding: 0.1rem 0.35rem;
		color: var(--text-muted);
		background: transparent;
		border: none;
	}
	.remove:hover {
		color: var(--danger);
	}
	.add {
		display: flex;
		gap: 0.4rem;
		padding: 0.2rem 0 0.3rem;
	}
	.add select {
		flex: 1;
		min-width: 0;
	}
	.examples {
		width: calc(100% - 2rem);
		margin: 0 1rem;
	}
	.examples th {
		border-bottom: 1px solid var(--border);
	}
	.examples td:not(:first-child),
	.examples th:not(:first-child) {
		text-align: right;
		font-variant-numeric: tabular-nums;
	}
	hr {
		border: none;
		border-top: 1px solid var(--border);
		margin: 0 1rem;
	}
	.status {
		padding: 0 1rem;
		font-size: 0.85rem;
		color: var(--warning);
	}
	footer {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.5rem;
		padding: 0 1rem;
	}
	footer .right {
		margin-left: auto;
	}
	button:disabled {
		opacity: 0.5;
		cursor: default;
	}
	@media (max-width: 30rem) {
		.tables {
			grid-template-columns: 1fr;
		}
	}
</style>
