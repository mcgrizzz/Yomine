<script lang="ts">
	// Term table (T029/T030, inline): the US1 mining surface, mirroring egui's
	// four-column layout — Term │ Sentence │ Frequency │ POS. The term shows its
	// reading as furigana above the lemma; the example sentence renders inline in
	// the row (always visible, no expander). Rows arrive already filtered and
	// sorted by `visibleTerms` (driven by `TableControls`, T037).
	import type { SentenceDto, Term } from '$lib/ipc';
	import { defaultDir, harmonic, type SortField } from '$lib/table';
	import { ignoredLemmas, toggleIgnore, posCatalog, tableSort } from '$lib/stores';
	import { posColor } from '$lib/pos';
	import Furigana from './Furigana.svelte';
	import SentenceView, { type Occurrence } from './SentenceView.svelte';

	let { terms, sentences }: { terms: Term[]; sentences: SentenceDto[] } = $props();

	// Column-header sorting (T061, egui `header.rs`): the Sentence and Frequency
	// headers ARE the sort controls. Clicking an inactive header activates it in
	// its natural direction; clicking it again reverses. The Sentence header owns
	// three modes — when active, a small chip shows the current one and clicking
	// the chip cycles Chronological → Sentence Count → Comprehension, keeping the
	// direction (egui's 🕒/#/📊 cycle icon). The active column header is
	// highlighted; hovering an inactive one previews its default direction arrow.
	const SENTENCE_MODES: { field: SortField; label: string; name: string }[] = [
		{ field: 'chronological', label: '🕒 Chronological', name: 'Chronological' },
		{ field: 'sentenceCount', label: '# Sentence Count', name: 'Sentence Count' },
		{ field: 'comprehension', label: '📊 Estimated Comprehension', name: 'Comprehension' }
	];
	const sentenceMode = $derived(SENTENCE_MODES.find((m) => m.field === $tableSort.field));
	const sentenceActive = $derived(sentenceMode !== undefined);
	const freqActive = $derived($tableSort.field === 'frequency');

	const dirArrow = (d: 'asc' | 'desc') => (d === 'asc' ? '⬆' : '⬇');
	const dirWord = (d: 'asc' | 'desc') => (d === 'asc' ? 'ascending' : 'descending');
	const sortedTip = (name: string) => `Sorted by ${name} in ${dirWord($tableSort.dir)} order`;

	function flipDir() {
		tableSort.update((s) => ({ ...s, dir: s.dir === 'asc' ? 'desc' : 'asc' }));
	}
	function clickSentence() {
		if (sentenceActive) flipDir();
		else tableSort.set({ field: 'chronological', dir: defaultDir('chronological') });
	}
	function clickFrequency() {
		if (freqActive) flipDir();
		else tableSort.set({ field: 'frequency', dir: defaultDir('frequency') });
	}
	function cycleSentence(e: MouseEvent) {
		e.stopPropagation();
		const i = SENTENCE_MODES.findIndex((m) => m.field === $tableSort.field);
		const next = SENTENCE_MODES[(i + 1) % SENTENCE_MODES.length].field;
		tableSort.update((s) => ({ field: next, dir: s.dir }));
	}

	// Term ignore (T059, egui parity): Ctrl/Cmd+Click the term to toggle ignore, or
	// right-click for the same as a menu. An ignored term stays visible but greyed —
	// the row only disappears on the next refresh; toggling again un-ignores it. The
	// menu closes on the next click/scroll anywhere.
	let menu = $state<{ x: number; y: number; lemma: string } | null>(null);

	function openMenu(e: MouseEvent, term: Term) {
		e.preventDefault();
		// Don't let the window `contextmenu` handler (which closes the menu) see this.
		e.stopPropagation();
		menu = { x: e.clientX, y: e.clientY, lemma: term.lemma_form };
	}

	// Ctrl (Win/Linux) or Cmd (macOS) + click toggles ignore; a plain click is left
	// alone so text selection still works. (On macOS Ctrl+Click opens the menu instead.)
	function termClick(e: MouseEvent, term: Term) {
		if (!e.ctrlKey && !e.metaKey) return;
		e.preventDefault();
		toggleIgnore(term.lemma_form);
	}

	// Track whether Ctrl/Cmd is held so the term shows a pointing-hand cursor while it
	// can be clicked-to-ignore (egui `set_cursor_icon(PointingHand)` on ctrl+hover).
	let ctrlHeld = $state(false);
	function trackMods(e: KeyboardEvent) {
		ctrlHeld = e.ctrlKey || e.metaKey;
	}

	function toggleFromMenu() {
		if (menu) toggleIgnore(menu.lemma);
		menu = null;
	}

	// key → display label ("Postposition" → "Particle"), from get_pos_catalog.
	const posLabels = $derived(Object.fromEntries($posCatalog.map((p) => [p.key, p.display_name])));

	// `Term.id` is not unique (the engine doesn't assign distinct ids); the pipeline
	// dedups by (lemma_form, hiragana reading), so this pair is a stable unique key.
	const termKey = (t: Term): string => `${t.lemma_form} ${t.lemma_reading}`;

	function freqLabel(term: Term): string {
		const v = harmonic(term);
		return v === Infinity ? '？' : String(v);
	}

	// Resolve a term's `sentence_references` (index, byte offset) to its example
	// sentences, matching egui's index-based lookup into the sentences array.
	// SentenceView owns the ◀ n/m ▶ browsing across them (T030b).
	function occurrencesOf(term: Term): Occurrence[] {
		const out: Occurrence[] = [];
		for (const [i, start] of term.sentence_references) {
			const sentence = sentences[i];
			if (sentence) out.push({ sentence, start });
		}
		return out;
	}
</script>

<div class="table">
	<div class="row head">
		<span>Term</span>
		<span class="head-cell">
			<button
				class="head-btn"
				class:active={sentenceActive}
				title={sentenceActive ? sortedTip(sentenceMode!.name) : 'Sort by Sentence'}
				onclick={clickSentence}
			>
				Sentence
				{#if sentenceActive}
					<span class="arrow active">{dirArrow($tableSort.dir)}</span>
				{:else}
					<span class="arrow hint">⇅</span>
					<span class="arrow preview">{dirArrow(defaultDir('chronological'))}</span>
				{/if}
			</button>
			{#if sentenceActive}
				<button
					class="mode"
					title="Cycle between Chronological, Sentence Count, and Comprehension"
					onclick={cycleSentence}>{sentenceMode!.label}</button
				>
			{/if}
		</span>
		<span class="num head-cell">
			<button
				class="head-btn"
				class:active={freqActive}
				title={freqActive ? sortedTip('Frequency') : 'Sort by Frequency'}
				onclick={clickFrequency}
			>
				Frequency
				{#if freqActive}
					<span class="arrow active">{dirArrow($tableSort.dir)}</span>
				{:else}
					<span class="arrow hint">⇅</span>
					<span class="arrow preview">{dirArrow(defaultDir('frequency'))}</span>
				{/if}
			</button>
		</span>
		<span>POS</span>
	</div>
	{#if terms.length === 0}
		<p class="no-match">No terms match the current filters.</p>
	{/if}
	{#each terms as term (termKey(term))}
		{@const occs = occurrencesOf(term)}
		<div class="row">
			<!-- svelte-ignore a11y_click_events_have_key_events -- Ctrl/Cmd+Click is a
			     mouse-modifier ignore toggle (egui parity); no keyboard equivalent. -->
			<span
					class="term"
					class:ignored={$ignoredLemmas.has(term.lemma_form)}
					class:ignorable={ctrlHeld}
					lang="ja"
					role="button"
					tabindex="-1"
					title={$ignoredLemmas.has(term.lemma_form)
						? 'Ctrl+Click to UNDO ignore'
						: 'Ctrl+Click to ignore'}
					onclick={(e) => termClick(e, term)}
					oncontextmenu={(e) => openMenu(e, term)}
					><Furigana surface={term.lemma_form} reading={term.lemma_reading} /></span
				>
			<div class="sentence">
				{#if occs.length > 0}
					<SentenceView occurrences={occs} {term} />
				{:else}
					<span class="empty">—</span>
				{/if}
			</div>
			<span class="num">{freqLabel(term)}</span>
			<span class="pos" style="color: {posColor(term.part_of_speech)}">
				{posLabels[term.part_of_speech] ?? term.part_of_speech}
			</span>
		</div>
	{/each}
</div>

{#if menu}
	<div class="ctx-menu" style="left: {menu.x}px; top: {menu.y}px;">
		<button type="button" onclick={toggleFromMenu}>
			{$ignoredLemmas.has(menu.lemma) ? 'Remove from ignore list' : 'Add to ignore list'}
		</button>
	</div>
{/if}

<svelte:window
	onclick={() => (menu = null)}
	onscrollcapture={() => (menu = null)}
	oncontextmenu={() => (menu = null)}
	onkeydown={trackMods}
	onkeyup={trackMods}
	onblur={() => (ctrlHeld = false)}
/>

<style>
	.table {
		display: flex;
		flex-direction: column;
		font-variant-numeric: tabular-nums;
	}
	.row {
		display: grid;
		grid-template-columns: minmax(7rem, max-content) 1fr 6rem 8rem;
		gap: 0.75rem;
		align-items: center;
		padding: 0.5rem;
		border-bottom: 1px solid var(--border);
	}
	.row:not(.head):hover {
		background: var(--bg-light);
	}
	.row.head {
		position: sticky;
		top: 0;
		background: var(--bg-dark);
		color: var(--comment);
		font-size: 0.8rem;
		text-transform: uppercase;
		letter-spacing: 0.03em;
	}
	/* Sortable column headers (T061, egui header.rs). */
	.head-cell {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
	}
	.head-cell.num {
		justify-content: flex-end;
	}
	.head-btn {
		display: inline-flex;
		align-items: center;
		gap: 0.25rem;
		padding: 0.1rem 0.35rem;
		background: transparent;
		border: none;
		border-radius: 3px;
		color: inherit;
		font: inherit;
		text-transform: inherit;
		letter-spacing: inherit;
		cursor: pointer;
	}
	/* Active-column highlight (egui draw_column_highlight: cyan @ 10%). */
	.head-btn.active {
		background: color-mix(in srgb, var(--cyan) 10%, transparent);
		color: var(--fg);
	}
	.head-btn:hover {
		background: var(--bg-light);
		color: var(--fg);
	}
	.arrow.active {
		color: var(--cyan);
	}
	/* Sortable-column affordance: a dim ⇅ that swaps to the default-direction
	   preview arrow on hover (egui sort_arrow_text). */
	.arrow.hint {
		opacity: 0.55;
	}
	.arrow.preview {
		display: none;
	}
	.head-btn:hover .arrow.hint {
		display: none;
	}
	.head-btn:hover .arrow.preview {
		display: inline;
	}
	/* The Sentence sort-mode chip (egui's small weak-text 🕒/#/📊 cycle label). */
	.mode {
		padding: 0.05rem 0.3rem;
		background: transparent;
		border: none;
		color: var(--comment);
		font-size: 0.7rem;
		text-transform: none;
		letter-spacing: normal;
		cursor: pointer;
		white-space: nowrap;
	}
	.mode:hover {
		color: var(--fg);
	}
	.num {
		text-align: right;
	}
	.term {
		font-size: 1.5rem;
		color: var(--red);
		line-height: 1.1;
	}
	/* Ignored-in-place: greyed until the next refresh drops the row (T059). */
	.term.ignored {
		color: var(--comment);
	}
	/* Pointing-hand while Ctrl/Cmd is held (the click-to-ignore affordance). */
	.term.ignorable {
		cursor: pointer;
	}
	.pos {
		font-size: 0.9rem;
	}
	.empty {
		color: var(--comment);
	}
	.no-match {
		margin: 0;
		padding: 1.5rem 0.5rem;
		color: var(--comment);
		text-align: center;
	}
	.ctx-menu {
		position: fixed;
		z-index: 100;
		background: var(--bg-dark);
		border: 1px solid var(--border);
		border-radius: 4px;
		box-shadow: 0 2px 8px rgba(0, 0, 0, 0.4);
		padding: 0.25rem;
	}
	.ctx-menu button {
		display: block;
		width: 100%;
		padding: 0.4rem 0.75rem;
		background: none;
		border: none;
		color: var(--fg);
		text-align: left;
		cursor: pointer;
		border-radius: 3px;
		font-size: 0.9rem;
	}
	.ctx-menu button:hover {
		background: var(--bg-light);
	}
</style>
