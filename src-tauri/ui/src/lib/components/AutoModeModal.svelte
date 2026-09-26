<script lang="ts">
	import { settingsDraft } from '$lib/settingsDraft.svelte';
	import { DEFAULT_HORIZON, frequencyPoints, pickPoints, POS_ALIASES } from '$lib/autopick';
	import type { AutoMine } from '$lib/ipc';
	import Modal from './Modal.svelte';
	import SettingsFooter from './SettingsFooter.svelte';
	import {
		autoModalOpen,
		defaultSettings,
		knowledge,
		posCatalog,
		setAutoMine,
		settings
	} from '$lib/stores';

	const JLPT_LEVELS = ['N5', 'N4', 'N3', 'N2', 'N1'];

	const copy = (a: AutoMine): AutoMine => ({
		stop: a.stop,
		limit: a.limit,
		min_score: a.min_score,
		max_cards: a.max_cards,
		pos_points: Object.fromEntries(
			Object.entries(a.pos_points).filter(([key]) => !(key in POS_ALIASES))
		),
		jlpt_points: Object.fromEntries(JLPT_LEVELS.map((l) => [l, a.jlpt_points[l] ?? 0]))
	});

	// Replaced from the saved settings each time the dialog opens.
	const form = settingsDraft<AutoMine>({
		open: autoModalOpen,
		initial: {
			stop: 'count',
			limit: 1,
			min_score: 0,
			max_cards: null,
			pos_points: {},
			jlpt_points: {}
		},
		load: () => {
			const saved = $settings?.auto_mine ?? $defaultSettings?.auto_mine;
			return saved && copy(saved);
		}
	});
	const draft = $derived(form.value);
	let adding = $state('');

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
	const editablePos = $derived($posCatalog.filter((p) => !(p.key in POS_ALIASES)));
	const posRows = $derived(
		editablePos.map((p) => p.key).filter((key) => key in draft.pos_points)
	);
	const choose = (stop: AutoMine['stop']) => () => (draft.stop = stop);
	const addable = $derived(editablePos.filter((p) => !(p.key in draft.pos_points)));

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

	function restoreDefault() {
		if ($defaultSettings) form.value = copy($defaultSettings.auto_mine);
	}
</script>

<Modal
	open={$autoModalOpen}
	title="Auto Mode"
	width="min(680px, 94%)"
	maxHeight="94%"
	onclose={form.request}
	oninteract={form.disarm}
>
	<div class="body">
		<p class="intro">
			Mines each new asbplayer video with your table filters. Cards include audio, a screenshot and
			the <code>yomine::auto</code> tag.
		</p>

		<section>
			<h3>Cards per video</h3>
			<!-- Each card's radio carries keyboard selection; clicks and typing anywhere in the
			     card select it too. -->
			<div class="choices" role="radiogroup" aria-label="Cards per video">
				<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
				<div
					class="choice"
					class:selected={draft.stop === 'count'}
					onclick={choose('count')}
					oninput={choose('count')}
				>
					<label class="choice-title">
						<input type="radio" value="count" bind:group={draft.stop} />
						Fixed count
					</label>
					<div class="row">
						Mine the best
						<input
							type="number"
							min="1"
							max="50"
							aria-label="Cards per video"
							bind:value={draft.limit}
						/>
						cards
					</div>
				</div>
				<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
				<div
					class="choice"
					class:selected={draft.stop === 'min_score'}
					onclick={choose('min_score')}
					oninput={choose('min_score')}
				>
					<label class="choice-title">
						<input type="radio" value="min_score" bind:group={draft.stop} />
						Score threshold
					</label>
					<div class="row">
						Mine terms scoring at least
						<input type="number" aria-label="Minimum score" bind:value={draft.min_score} />
					</div>
					<div class="row">
						Up to
						<input
							type="number"
							min="1"
							aria-label="Card cap"
							disabled={draft.max_cards === null}
							value={draft.max_cards ?? ''}
							oninput={(e) => (draft.max_cards = e.currentTarget.valueAsNumber)}
						/>
						cards
						<label class="check">
							<input
								type="checkbox"
								checked={draft.max_cards === null}
								onchange={(e) => (draft.max_cards = e.currentTarget.checked ? null : 40)}
							/>
							No cap
						</label>
					</div>
				</div>
			</div>
		</section>

		<section>
			<div class="section-head">
				<h3>Scoring</h3>
				<span class="hint">Score = frequency + word type + JLPT. Higher scores are mined first.</span>
			</div>
			<div class="scoring">
				<div class="group">
					<div class="group-head">Word type</div>
					<div
						class="word-types"
						style:grid-template-rows="repeat({Math.ceil(posRows.length / 2)}, auto)"
					>
						{#each posRows as key (key)}
							<div class="point-row">
								<span class="name" title={posName(key)}>{posName(key)}</span>
								<input
									type="number"
									min="-50"
									max="50"
									aria-label="{posName(key)} points"
									bind:value={draft.pos_points[key]}
								/>
								<button
									class="remove"
									title="Remove {posName(key)}"
									aria-label="Remove {posName(key)}"
									onclick={() => removePos(key)}>✕</button
								>
							</div>
						{/each}
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
				</div>

				<div class="group">
					<div class="group-head">JLPT level</div>
					{#each JLPT_LEVELS as level (level)}
						<div class="point-row">
							<span class="name">{level}</span>
							<input
								type="number"
								min="-50"
								max="50"
								aria-label="{level} points"
								bind:value={draft.jlpt_points[level]}
							/>
						</div>
					{/each}
				</div>
			</div>

			<details class="how">
				<summary>How scoring works</summary>
				<div class="how-body">
					<p>
						Frequency is 40 points up to your horizon, <strong>rank {horizon.toLocaleString()}</strong>,
						and 20 fewer per tenfold step past it. The horizon grows with your Anki cards. Word types
						without a row score 0, and a video gets fewer cards when your filters leave fewer terms.
					</p>
					<table class="examples">
						<thead><tr><th>Example</th><th>Frequency</th><th>Score</th></tr></thead>
						<tbody>
							{#each examples as e (e.label)}
								<tr><td>{e.label}</td><td>{e.freq}</td><td>{e.total}</td></tr>
							{/each}
						</tbody>
					</table>
				</div>
			</details>
		</section>
		{#if !valid}
			<p class="invalid">
				⚠ Cards per video must be 1–50, the cap at least 1, and points between -50 and 50
			</p>
		{/if}
	</div>

	{#snippet footer()}
		<SettingsFooter
			{form}
			invalid={!valid}
			onsave={save}
			oncancel={form.revert}
			onrestore={restoreDefault}
		/>
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
	code {
		font-size: 0.85em;
	}
	section {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}
	h3 {
		margin: 0;
		font-size: 0.9rem;
		font-weight: 600;
	}
	.section-head {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		column-gap: 0.75rem;
	}
	.hint {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.choices {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.6rem;
	}
	.choice {
		display: flex;
		flex-direction: column;
		gap: 0.45rem;
		padding: 0.6rem 0.75rem;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		cursor: pointer;
	}
	.choice:hover:not(.selected) {
		border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
	}
	.choice.selected {
		border-color: var(--accent);
		background: color-mix(in srgb, var(--accent) 7%, transparent);
	}
	.choice-title {
		display: flex;
		align-items: center;
		gap: 0.45rem;
		font-weight: 600;
		cursor: pointer;
	}
	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.4rem;
		padding-left: 1.55rem;
		font-size: 0.875rem;
	}
	.check {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
		margin-left: 0.4rem;
		cursor: pointer;
	}
	input[type='number'] {
		width: 4.25rem;
		padding: 0.2rem 0.45rem;
		background: var(--bg-raised);
		color: var(--text);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		font-variant-numeric: tabular-nums;
		cursor: text;
	}
	.scoring {
		display: grid;
		grid-template-columns: minmax(0, 2.4fr) minmax(0, 1fr);
		gap: 1.5rem;
		align-items: start;
	}
	.group {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}
	.group-head {
		padding-bottom: 0.25rem;
		margin-bottom: 0.2rem;
		font-size: 0.8rem;
		color: var(--text-muted);
		border-bottom: 1px solid var(--border);
	}
	.word-types {
		display: grid;
		grid-auto-flow: column;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		column-gap: 1rem;
	}
	/* Name, points and remove share one track set, so inputs align across every column. */
	.point-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 4.25rem 1.75rem;
		align-items: center;
		column-gap: 0.4rem;
		min-height: 2rem;
		font-size: 0.85rem;
	}
	.name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
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
		margin-top: 0.35rem;
		max-width: 20rem;
	}
	.add select {
		flex: 1;
		min-width: 0;
	}
	.how {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.how summary {
		width: fit-content;
		cursor: pointer;
		color: var(--text);
		font-size: 0.85rem;
	}
	.how-body {
		display: grid;
		grid-template-columns: minmax(0, 1.3fr) minmax(0, 1fr);
		gap: 1.25rem;
		align-items: start;
		padding-top: 0.5rem;
	}
	.how strong {
		color: var(--text);
		font-weight: 600;
	}
	.examples {
		width: 100%;
		border-collapse: collapse;
	}
	.examples th {
		text-align: left;
		font-weight: normal;
		border-bottom: 1px solid var(--border);
	}
	.examples td,
	.examples th {
		padding: 0.15rem 0.3rem;
		white-space: nowrap;
	}
	.examples td:not(:first-child),
	.examples th:not(:first-child) {
		text-align: right;
		font-variant-numeric: tabular-nums;
	}
	.invalid {
		font-size: 0.85rem;
		color: var(--danger);
	}
	@media (max-width: 40rem) {
		.choices,
		.scoring,
		.how-body {
			grid-template-columns: 1fr;
		}
	}
</style>
