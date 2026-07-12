<script lang="ts">
	import type { SentenceDto, Term } from '$lib/ipc';
	import { defaultDir, harmonic, termKey, textMatches, type SortField } from '$lib/table';
	import {
		addedTerms,
		ankiStatus,
		cancelQueue,
		clearSelection,
		ignoredLemmas,
		mediaMissing,
		mineQueue,
		mineQueueState,
		mineTerm,
		minedNoteIds,
		minedTerms,
		miningTerm,
		normalizeSentence,
		openInAnki,
		playerBusy,
		playerStatus,
		posCatalog,
		retryMedia,
		selectedTerms,
		setSelected,
		settings,
		tableSearch,
		tableSort,
		toggleIgnore,
		toggleSelected,
		yomitanReachable,
		type QueueItem
	} from '$lib/stores';
	import { posColor } from '$lib/pos';
	import DefinitionPopover from './DefinitionPopover.svelte';
	import Furigana from './Furigana.svelte';
	import SentenceConflictModal, { type BatchEntry } from './SentenceConflictModal.svelte';
	import SentenceView, { type Occurrence } from './SentenceView.svelte';

	let { terms, sentences }: { terms: Term[]; sentences: SentenceDto[] } = $props();

	// The column headers ARE the sort controls; the Sentence header owns three
	// modes, cycled via its chip while active.
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

	// Ignored terms stay visible but greyed; the row only disappears on the next
	// refresh, so the toggle is undoable in place (egui parity).
	let menu = $state<{ x: number; y: number; lemma: string } | null>(null);

	function openMenu(e: MouseEvent, term: Term) {
		e.preventDefault();
		// Don't let the window `contextmenu` handler (which closes the menu) see this.
		e.stopPropagation();
		menu = { x: e.clientX, y: e.clientY, lemma: term.lemma_form };
	}

	let defPopover = $state<{ term: Term; anchor: DOMRect } | null>(null);
	let hovered: { term: Term; el: HTMLElement } | null = null;

	// Shift+Hover definition popover (issue #113). Pressing Shift while already
	// hovering is handled in trackMods — mouseenter won't re-fire for it.
	function openDefinition(h: { term: Term; el: HTMLElement }) {
		if (!$yomitanReachable) return;
		defPopover = { term: h.term, anchor: h.el.getBoundingClientRect() };
	}

	function termEnter(e: MouseEvent, term: Term) {
		hovered = { term, el: e.currentTarget as HTMLElement };
		if (e.shiftKey) openDefinition(hovered);
	}

	// Ctrl (Win/Linux) or Cmd (macOS) + click toggles ignore; a plain click is left
	// alone so text selection still works. (On macOS Ctrl+Click opens the menu instead.)
	function termClick(e: MouseEvent, term: Term) {
		if (!e.ctrlKey && !e.metaKey) return;
		e.preventDefault();
		toggleIgnore(term.lemma_form);
	}

	// Pointing-hand cursor while Ctrl/Cmd is held (the click-to-ignore affordance).
	let ctrlHeld = $state(false);
	function trackMods(e: KeyboardEvent) {
		ctrlHeld = e.ctrlKey || e.metaKey;
		if (e.key === 'Shift' && e.shiftKey && !e.repeat && hovered) openDefinition(hovered);
	}

	function toggleFromMenu() {
		if (menu) toggleIgnore(menu.lemma);
		menu = null;
	}

	// key → display label ("Postposition" → "Particle"), from get_pos_catalog.
	const posLabels = $derived(Object.fromEntries($posCatalog.map((p) => [p.key, p.display_name])));

	function freqLabel(term: Term): string {
		const v = harmonic(term);
		return v === Infinity ? '？' : String(v);
	}

	function occurrencesOf(term: Term): Occurrence[] {
		const out: Occurrence[] = [];
		for (const [i, start] of term.sentence_references) {
			const sentence = sentences[i];
			if (sentence) out.push({ sentence, start });
		}
		return out;
	}

	// Each row's occurrence index, bound up so the mine button and the
	// search-jump below target the sentence on display.
	let occIdx = $state<Record<string, number>>({});

	// A search matching inside a sentence jumps the row to that sentence.
	$effect(() => {
		const q = $tableSearch.trim();
		if (!q) return;
		for (const term of terms) {
			const occs = occurrencesOf(term);
			const match = occs.findIndex((o) => textMatches(o.sentence.text, q));
			if (match >= 0) occIdx[termKey(term)] = match;
		}
	});

	const isMined = (t: Term): boolean =>
		$minedTerms.has(t.lemma_form) ||
		$addedTerms.has(t.lemma_form) ||
		$addedTerms.has(t.surface_form);

	function mine(term: Term, occs: Occurrence[]) {
		const occ = occs[Math.min(occIdx[termKey(term)] ?? 0, occs.length - 1)];
		const ts = occ?.sentence.timestamp ?? null;
		// asbplayer enrichment needs asbplayer active (same rule as seeking) + a cue.
		const via =
			$playerStatus.mode === 'asbplayer' && $playerStatus.ws_clients > 0 && ts !== null
				? 'asbplayer'
				: 'direct';
		void mineTerm(term, occ?.sentence.text ?? '', ts, via);
	}

	function retry(term: Term, occs: Occurrence[]) {
		const occ = occs[Math.min(occIdx[termKey(term)] ?? 0, occs.length - 1)];
		void retryMedia(term, occ?.sentence.timestamp ?? null);
	}

	const showJlpt = $derived(
		($settings?.show_jlpt_tags ?? true) && terms.some((t) => t.jlpt_level !== null)
	);

	const canMine = $derived($yomitanReachable && $ankiStatus.connected);
	const selectableKeys = $derived(terms.filter((t) => !isMined(t)).map(termKey));
	const allSelected = $derived(
		selectableKeys.length > 0 && selectableKeys.every((k) => $selectedTerms.has(k))
	);
	const someSelected = $derived(selectableKeys.some((k) => $selectedTerms.has(k)));

	function rowClick(e: MouseEvent, term: Term) {
		if (!canMine || isMined(term)) return;
		if (e.ctrlKey || e.metaKey) return;
		if ((e.target as HTMLElement).closest('button, input, a')) return;
		if (window.getSelection()?.toString()) return;
		toggleSelected(termKey(term));
	}

	let batchEntries = $state<BatchEntry[] | null>(null);

	function startBatch() {
		const entries: BatchEntry[] = terms
			.filter((t) => $selectedTerms.has(termKey(t)) && !isMined(t))
			.map((t) => {
				const key = termKey(t);
				const occs = occurrencesOf(t);
				const occ = occs[Math.min(occIdx[key] ?? 0, occs.length - 1)];
				const seen = new Set([normalizeSentence(occ?.sentence.text ?? '')]);
				const alternatives = occs.flatMap((o, idx) => {
					const k = normalizeSentence(o.sentence.text);
					if (seen.has(k)) return [];
					seen.add(k);
					return [{ idx, sentence: o.sentence.text, timestamp: o.sentence.timestamp }];
				});
				return {
					term: t,
					key,
					sentence: occ?.sentence.text ?? '',
					timestamp: occ?.sentence.timestamp ?? null,
					explicit: occIdx[key] !== undefined,
					alternatives
				};
			});
		const keys = entries.map((e) => normalizeSentence(e.sentence)).filter((s) => s !== '');
		if (new Set(keys).size === keys.length) {
			void mineQueue(entries.map(({ term, sentence, timestamp }) => ({ term, sentence, timestamp })));
			return;
		}
		batchEntries = entries;
	}

	function conflictsResolved(items: QueueItem[], occIdxPatch: Record<string, number>) {
		// Sync the rows to any reassigned occurrences so display = mined.
		for (const [key, idx] of Object.entries(occIdxPatch)) occIdx[key] = idx;
		batchEntries = null;
		void mineQueue(items);
	}
</script>

{#if batchEntries}
	<SentenceConflictModal
		entries={batchEntries}
		ondone={conflictsResolved}
		oncancel={() => (batchEntries = null)}
	/>
{/if}

{#if $mineQueueState}
	<div class="bulk-bar">
		<span class="bulk-info">
			Mining {$mineQueueState.done + 1}/{$mineQueueState.total} 「{$mineQueueState.current}」
		</span>
		<button class="bulk-btn" onclick={cancelQueue}>Cancel</button>
	</div>
{:else if $selectedTerms.size > 0}
	<div class="bulk-bar">
		<span class="bulk-info">{$selectedTerms.size} selected</span>
		{#if canMine}
			<button
				class="bulk-btn primary"
				disabled={$miningTerm !== null || $playerBusy}
				title="Mine the selected terms one by one, in timestamp order"
				onclick={startBatch}>Mine {$selectedTerms.size}</button
			>
		{/if}
		<button class="bulk-btn" onclick={clearSelection}>Clear</button>
	</div>
{/if}

<div class="table" class:no-jlpt={!showJlpt}>
	<div class="row head">
		<span class="sel">
			{#if canMine && selectableKeys.length > 0}
				<input
					type="checkbox"
					checked={allSelected}
					indeterminate={someSelected && !allSelected}
					onchange={() => setSelected(selectableKeys, !allSelected)}
					title="Select all visible terms"
					aria-label="Select all visible terms"
				/>
			{/if}
		</span>
		<span>Term</span>
		{#if showJlpt}
			<span class="jlpt-cell">JLPT</span>
		{/if}
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
		{@const key = termKey(term)}
		<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions --
		     row click mirrors the row's checkbox, which stays keyboard-accessible. -->
		<div
			class="row"
			class:selected={$selectedTerms.has(key)}
			onclick={(e) => rowClick(e, term)}
		>
			<span class="sel">
				{#if canMine && !isMined(term)}
					<input
						type="checkbox"
						checked={$selectedTerms.has(key)}
						onchange={() => toggleSelected(key)}
						aria-label={`Select ${term.lemma_form}`}
					/>
				{/if}
			</span>
			<span class="term-cell">
				<!-- svelte-ignore a11y_click_events_have_key_events -- Ctrl/Cmd+Click is a
				     mouse-modifier ignore toggle (egui parity); no keyboard equivalent. -->
				<span
						class="term"
						class:mined-term={isMined(term)}
						class:ignored={$ignoredLemmas.has(term.lemma_form)}
						class:ignorable={ctrlHeld}
						lang="ja"
						role="button"
						tabindex="-1"
						title={($yomitanReachable ? 'Shift+Hover for definition · ' : '') +
							($ignoredLemmas.has(term.lemma_form)
								? 'Ctrl+Click to UNDO ignore'
								: 'Ctrl+Click to ignore')}
						onclick={(e) => termClick(e, term)}
						oncontextmenu={(e) => openMenu(e, term)}
						onmouseenter={(e) => termEnter(e, term)}
						onmouseleave={() => (hovered = null)}
						><Furigana surface={term.lemma_form} reading={term.lemma_reading} /></span
					>
				{#if isMined(term)}
					{@const noteId = $minedNoteIds[term.lemma_form]}
					{#if noteId !== undefined && $mediaMissing.has(term.lemma_form)}
						<button
							class="chip warn"
							disabled={$miningTerm !== null || $playerBusy}
							title="Card is in Anki, but asbplayer never added the audio/screenshot — click to retry"
							onclick={() => retry(term, occs)}
						>
							{$miningTerm === term.lemma_form ? '…' : '⚠'}
						</button>
					{:else if noteId !== undefined}
						<button
							class="chip mined openable"
							title="In Anki — click to open the card"
							onclick={() => openInAnki(noteId)}>✓</button
						>
					{:else}
						<span class="chip mined" title="This term already has a recent Anki card">✓</span>
					{/if}
				{:else if $yomitanReachable && $ankiStatus.connected}
					<!-- Mining needs BOTH: Yomitan renders the card, AnkiConnect stores it. -->
					<button
						class="chip mine"
						disabled={$miningTerm !== null || $playerBusy}
						title={$playerBusy && $miningTerm === null
							? 'Waiting for asbplayer to finish recording the mined line…'
							: 'Create an Anki card from the displayed sentence'}
						onclick={() => mine(term, occs)}
					>
						{$miningTerm === term.lemma_form ? '…' : '+'}
					</button>
				{/if}
			</span>
			{#if showJlpt}
				<span class="jlpt-cell">
					{#if term.jlpt_level}
						<span class="jlpt-chip">{term.jlpt_level}</span>
					{/if}
				</span>
			{/if}
			<div class="sentence">
				{#if occs.length > 0}
					<SentenceView occurrences={occs} {term} bind:currentIndex={occIdx[key]} />
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

{#if defPopover}
	{@const popTerm = defPopover.term}
	<DefinitionPopover
		term={popTerm}
		anchor={defPopover.anchor}
		scale={$settings?.definition_scale ?? 1}
		showMine={canMine && !isMined(popTerm)}
		mineDisabled={$miningTerm !== null || $playerBusy}
		onmine={() => mine(popTerm, occurrencesOf(popTerm))}
		onclose={() => (defPopover = null)}
	/>
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
	/* One shared track list (rows subgrid it) so the max-content term column is
	   sized globally — per-row grids each size their own and misalign. */
	.table {
		display: grid;
		grid-template-columns: 1.5rem minmax(7rem, max-content) 3rem 1fr 6rem 8rem;
		/* Subgrid rows inherit this; a gap on .row would be ignored. */
		column-gap: 0.75rem;
		font-variant-numeric: tabular-nums;
	}
	.table.no-jlpt {
		grid-template-columns: 1.5rem minmax(7rem, max-content) 1fr 6rem 8rem;
	}
	.row {
		grid-column: 1 / -1;
		display: grid;
		grid-template-columns: subgrid;
		align-items: center;
		padding: 0.5rem;
		border-bottom: 1px solid var(--border);
	}
	.sel {
		display: inline-flex;
		align-items: center;
		justify-content: center;
	}
	.sel input {
		cursor: pointer;
	}
	.row:not(.head):hover {
		background: var(--bg-light);
	}
	.row.selected {
		background: color-mix(in srgb, var(--cyan) 7%, transparent);
	}
	.row.selected:hover {
		background: color-mix(in srgb, var(--cyan) 12%, transparent);
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
	/* Sortable column headers. */
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
	/* Active-column highlight. */
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
	   preview arrow on hover. */
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
	.term-cell {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
	}
	.jlpt-cell {
		text-align: center;
	}
	.term {
		font-size: 1.5rem;
		color: var(--red);
		line-height: 1.1;
	}
	/* Kept above .ignored so an ignored term still greys out. */
	.term.mined-term {
		color: var(--green);
	}
	/* Mine (+) and mined (✓) share one footprint so the swap doesn't shift layout. */
	.chip {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 1.5rem;
		height: 1.5rem;
		padding: 0;
		font-size: 0.95rem;
		line-height: 1;
		border-radius: var(--radius);
	}
	.mine {
		color: var(--cyan);
		background: var(--bg-light);
		border: 1px solid var(--border);
		cursor: pointer;
	}
	.mine:hover:not(:disabled) {
		background: var(--bg-lighter);
		border-color: var(--cyan);
	}
	.mine:disabled {
		opacity: 0.5;
		cursor: default;
	}
	.mined {
		color: var(--green);
		background: color-mix(in srgb, var(--green) 12%, transparent);
		border: 1px solid color-mix(in srgb, var(--green) 35%, transparent);
		cursor: help;
	}
	.mined.openable {
		cursor: pointer;
	}
	.mined.openable:hover {
		background: color-mix(in srgb, var(--green) 25%, transparent);
	}
	/* Note exists but asbplayer media never landed — click retries the enrichment. */
	.warn {
		color: var(--yellow);
		background: color-mix(in srgb, var(--yellow) 12%, transparent);
		border: 1px solid color-mix(in srgb, var(--yellow) 35%, transparent);
		cursor: pointer;
	}
	.warn:hover:not(:disabled) {
		background: color-mix(in srgb, var(--yellow) 25%, transparent);
	}
	.warn:disabled {
		opacity: 0.5;
		cursor: default;
	}
	/* Ignored-in-place: greyed until the next refresh drops the row. */
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
	.jlpt-chip {
		padding: 0.05rem 0.3rem;
		font-size: 0.7rem;
		color: var(--cyan);
		background: color-mix(in srgb, var(--cyan) 10%, transparent);
		border: 1px solid color-mix(in srgb, var(--cyan) 35%, transparent);
		border-radius: var(--radius);
		white-space: nowrap;
	}
	/* Floating selection/queue bar (issue #114): fixed so appearing/disappearing
	   never reflows the table (the header would jump under the pointer). */
	.bulk-bar {
		position: fixed;
		bottom: 1.25rem;
		left: 50%;
		transform: translateX(-50%);
		z-index: 40;
		display: flex;
		align-items: center;
		gap: 0.6rem;
		max-width: 90vw;
		padding: 0.45rem 0.9rem;
		background: var(--bg-dark);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
		font-size: 0.85rem;
	}
	.bulk-info {
		color: var(--fg);
	}
	.bulk-btn {
		cursor: pointer;
		padding: 0.25rem 0.6rem;
		background: var(--bg-dark);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		color: var(--fg);
	}
	.bulk-btn.primary {
		color: var(--cyan);
		border-color: color-mix(in srgb, var(--cyan) 35%, transparent);
	}
	.bulk-btn:disabled {
		opacity: 0.5;
		cursor: default;
	}
	.empty {
		color: var(--comment);
	}
	.no-match {
		grid-column: 1 / -1;
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
