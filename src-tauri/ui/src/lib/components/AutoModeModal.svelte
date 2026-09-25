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
			Mines each new asbplayer video as it loads, using the terms your table filters show, and records
			audio and screenshots from the video tab. Its notes are tagged <code>yomine::auto</code>.
		</p>

		<section>
			<h3>Cards per video</h3>
			<label class="option">
				<input type="radio" value="count" bind:group={draft.stop} />
				<span>Mine the best</span>
				<input
					type="number"
					min="1"
					max="50"
					aria-label="Cards per video"
					disabled={draft.stop !== 'count'}
					bind:value={draft.limit}
				/>
				<span>cards</span>
			</label>
			<label class="option">
				<input type="radio" value="min_score" bind:group={draft.stop} />
				<span>Mine every term scoring at least</span>
				<input
					type="number"
					aria-label="Minimum score"
					disabled={draft.stop !== 'min_score'}
					bind:value={draft.min_score}
				/>
			</label>
			{#if draft.stop === 'min_score'}
				<div class="option nested">
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
					<label class="check">
						<input
							type="checkbox"
							checked={draft.max_cards === null}
							onchange={(e) => (draft.max_cards = e.currentTarget.checked ? null : 40)}
						/>
						No cap
					</label>
				</div>
			{/if}
			<p class="hint">Fewer when your table filters leave less.</p>
		</section>

		<section>
			<h3>Scoring</h3>
			<p class="formula">Score = frequency + word type + JLPT</p>
			<p class="hint">
				Frequency is 40 up to your horizon, <strong>rank {horizon.toLocaleString()}</strong>, and 20
				less per tenfold step past it. The horizon grows with your Anki cards. Highest score is mined
				first.
			</p>
			<table class="examples">
				<thead><tr><th>Example</th><th>Frequency</th><th>Score</th></tr></thead>
				<tbody>
					{#each examples as e (e.label)}
						<tr><td>{e.label}</td><td>{e.freq}</td><td>{e.total}</td></tr>
					{/each}
				</tbody>
			</table>

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
			{#if addable.length > 0}
				<div class="add">
					<select bind:value={adding} aria-label="Word type to add">
						<option value="">Add word type…</option>
						{#each addable as p (p.key)}
							<option value={p.key}>{p.display_name}</option>
						{/each}
					</select>
					<button disabled={!adding} onclick={addPos}>Add</button>
				</div>
			{/if}
			<p class="hint">Word types without a row score 0.</p>
		</section>
		{#if !valid}
			<p class="invalid">
				⚠ Cards per video must be 1–50, the cap at least 1, and points between -50 and 50
			</p>
		{/if}
	</div>

	{#snippet footer()}
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
		gap: 1rem;
		padding: 0 1rem 0.5rem;
	}
	p {
		margin: 0;
	}
	.intro {
		font-size: 0.9rem;
	}
	.intro code {
		font-size: 0.85em;
	}
	section {
		display: flex;
		flex-direction: column;
		gap: 0.45rem;
	}
	h3 {
		margin: 0;
		font-size: 0.75rem;
		font-weight: 600;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: var(--text-muted);
	}
	.option {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.5rem;
	}
	.nested {
		padding-left: 1.6rem;
	}
	.check {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
		margin-left: 0.4rem;
	}
	input[type='number'] {
		width: 4rem;
		padding: 0.2rem 0.45rem;
		background: var(--bg-raised);
		color: var(--text);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		font-variant-numeric: tabular-nums;
	}
	.formula {
		font-size: 0.9rem;
		font-weight: 600;
	}
	.hint {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.hint strong {
		color: var(--text);
		font-weight: 600;
	}
	.invalid {
		font-size: 0.85rem;
		color: var(--danger);
	}
	.tables {
		display: grid;
		grid-template-columns: 3fr 2fr;
		gap: 1.25rem;
		align-items: start;
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
		border-bottom: 1px solid var(--border);
	}
	td,
	th {
		padding: 0.15rem 0.3rem;
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
		max-width: 20rem;
	}
	.add select {
		flex: 1;
		min-width: 0;
	}
	.examples td:not(:first-child),
	.examples th:not(:first-child) {
		text-align: right;
		font-variant-numeric: tabular-nums;
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
