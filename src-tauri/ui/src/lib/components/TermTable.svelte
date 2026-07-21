<script lang="ts">
	import type { SentenceDto, Term } from '$lib/ipc';
	import {
		defaultDir,
		harmonic,
		normalizeColumns,
		termKey,
		textMatches,
		type ColumnId,
		type SortField
	} from '$lib/table';
	import {
		addedTerms,
		ankiStatus,
		asbContext,
		cancelQueue,
		cardFormats,
		clearSelection,
		fileResult,
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
		queuedMineOptions,
		queueWithEntry,
		retryMedia,
		selectedTerms,
		setQueuedFormat,
		setSelected,
		setTableColumns,
		settings,
		tableSearch,
		tableSort,
		toggleIgnore,
		toggleSelected,
		yomitanReachable,
		type QueueItem
	} from '$lib/stores';
	import { Menu } from '@tauri-apps/api/menu';
	import { furiganaText } from '$lib/furigana';
	import { posColor } from '$lib/pos';
	import DefinitionPopover from './DefinitionPopover.svelte';
	import Furigana from './Furigana.svelte';
	import SentenceConflictModal, { type BatchEntry } from './SentenceConflictModal.svelte';
	import SentenceView, {
		termCoversSegment,
		termHighlightText,
		type Occurrence,
		type SegmentLookup
	} from './SentenceView.svelte';

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
	const jlptActive = $derived($tableSort.field === 'jlpt');

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
	function clickJlpt() {
		if (jlptActive) flipDir();
		else tableSort.set({ field: 'jlpt', dir: defaultDir('jlpt') });
	}
	function cycleSentence(e: MouseEvent) {
		e.stopPropagation();
		const i = SENTENCE_MODES.findIndex((m) => m.field === $tableSort.field);
		const next = SENTENCE_MODES[(i + 1) % SENTENCE_MODES.length].field;
		tableSort.update((s) => ({ field: next, dir: s.dir }));
	}

	// Ignored terms stay visible but greyed; the row only disappears on the next
	// refresh, so the toggle is undoable in place (egui parity).
	async function openMenu(e: MouseEvent, term: Term) {
		e.preventDefault();
		const lemma = term.lemma_form;
		const menu = await Menu.new({
			items: [
				{
					id: 'copy',
					text: 'Copy',
					action: () => void navigator.clipboard.writeText(lemma)
				},
				{
					id: 'copy-furigana',
					text: 'Copy with furigana',
					action: () =>
						void navigator.clipboard.writeText(furiganaText(lemma, term.lemma_reading))
				},
				{ item: 'Separator' },
				{
					id: 'ignore',
					text: $ignoredLemmas.has(lemma) ? 'Remove from ignore list' : 'Add to ignore list',
					action: () => toggleIgnore(lemma)
				}
			]
		});
		await menu.popup();
	}

	let defPopover = $state<{
		text: string;
		label: string;
		anchor: DOMRect;
		mineable: { term: Term; occs: Occurrence[] } | null;
	} | null>(null);
	let hovered: (() => void) | null = null;

	// Shift+Hover definition popover (issue #113). Pressing Shift while already
	// hovering is handled in trackMods — mouseenter won't re-fire for it.
	function termEnter(e: MouseEvent, term: Term) {
		const el = e.currentTarget as HTMLElement;
		const open = () => {
			if (!$yomitanReachable) return;
			defPopover = {
				text: term.lemma_form,
				label: term.lemma_form,
				anchor: el.getBoundingClientRect(),
				mineable: { term, occs: occurrencesOf(term) }
			};
		};
		hovered = open;
		if (e.shiftKey) open();
	}

	function segmentLookup(req: SegmentLookup) {
		if (!$yomitanReachable) return;
		let mineable: { term: Term; occs: Occurrence[] } | null = null;
		outer: for (const t of $fileResult?.terms ?? terms) {
			for (const [sid, start] of t.sentence_references) {
				if (sid !== req.sentence.id || !termCoversSegment(t, start, req.seg)) continue;
				mineable = { term: t, occs: [{ sentence: req.sentence, start }] };
				break outer;
			}
		}
		defPopover = { text: req.text, label: req.label, anchor: req.anchor, mineable };
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
		if (e.key === 'Shift' && e.shiftKey && !e.repeat && hovered) hovered();
		if (e.key === 'Escape') {
			editColumns = false;
			confirmMine = null;
		}
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

	let confirmMine = $state<{ term: Term; occs: Occurrence[] } | null>(null);

	function mineClicked(term: Term, occs: Occurrence[]) {
		if ($selectedTerms.size > 0) confirmMine = { term, occs };
		else mine(term, occs);
	}

	function confirmedMine() {
		if (!confirmMine) return;
		const { term, occs } = confirmMine;
		confirmMine = null;
		mine(term, occs);
	}

	function mine(term: Term, occs: Occurrence[], entryIndex?: number, formatName?: string) {
		const occ = occs[Math.min(occIdx[termKey(term)] ?? 0, occs.length - 1)];
		const ts = occ?.sentence.timestamp ?? null;
		// asbplayer enrichment needs asbplayer active (same rule as seeking) + a cue.
		const via =
			$playerStatus.mode === 'asbplayer' && $playerStatus.ws_clients > 0 && ts !== null
				? 'asbplayer'
				: 'direct';
		const surface = occ ? termHighlightText(term, occ) : term.surface_form;
		void mineTerm(term, occ?.sentence.text ?? '', ts, via, surface, entryIndex, formatName);
	}

	function retry(term: Term, occs: Occurrence[]) {
		const occ = occs[Math.min(occIdx[termKey(term)] ?? 0, occs.length - 1)];
		void retryMedia(term, occ?.sentence.timestamp ?? null);
	}

	const COLUMN_TRACKS: Record<ColumnId, string> = {
		term: 'minmax(7rem, max-content)',
		jlpt: 'minmax(3rem, max-content)',
		sentence: '1fr',
		frequency: '6rem',
		pos: '8rem'
	};
	const COLUMN_LABELS: Record<ColumnId, string> = {
		term: 'Term',
		jlpt: 'JLPT',
		sentence: 'Sentence',
		frequency: 'Frequency',
		pos: 'POS'
	};
	const columns = $derived(
		normalizeColumns($settings?.table_columns, $settings?.show_jlpt_tags ?? true)
	);
	const hasJlpt = $derived(terms.some((t) => t.jlpt_level !== null));
	const visibleCols = $derived(
		columns.filter((c) => c.visible && (c.id !== 'jlpt' || hasJlpt)).map((c) => c.id)
	);

	let editColumns = $state(false);
	let editCols = $state<{ id: ColumnId; visible: boolean }[]>([]);
	let dragId = $state<ColumnId | null>(null);

	async function openHeaderMenu(e: MouseEvent) {
		e.preventDefault();
		const menu = await Menu.new({
			items: [{ id: 'edit-columns', text: 'Edit columns…', action: startEditColumns }]
		});
		await menu.popup();
	}

	function startEditColumns() {
		editCols = columns.map((c) => ({ ...c }));
		editColumns = true;
	}

	// Pointer-based drag (HTML5 DnD aborts when the dragged node is reordered
	// mid-drag in WebView2): capture on the pill, retarget via elementFromPoint.
	function pillDown(e: PointerEvent, id: ColumnId) {
		if ((e.target as HTMLElement).closest('input')) return;
		dragId = id;
		(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
	}

	function pillMove(e: PointerEvent) {
		if (dragId === null) return;
		const over = document
			.elementFromPoint(e.clientX, e.clientY)
			?.closest('[data-col]')
			?.getAttribute('data-col') as ColumnId | null;
		if (!over || over === dragId) return;
		const from = editCols.findIndex((c) => c.id === dragId);
		const to = editCols.findIndex((c) => c.id === over);
		const [moved] = editCols.splice(from, 1);
		editCols.splice(to, 0, moved);
	}

	function pillUp() {
		if (dragId === null) return;
		dragId = null;
		commitColumns();
	}

	const commitColumns = () => void setTableColumns(editCols);

	const renderCols = $derived(editColumns ? editCols.map((c) => c.id) : visibleCols);
	const gridTemplate = $derived(
		['1.5rem', ...renderCols.map((id) => COLUMN_TRACKS[id])].join(' ')
	);

	// Mining needs Yomitan (renders the card) + AnkiConnect (stores it).
	const canMine = $derived($yomitanReachable && $ankiStatus.connected);
	// Only asbplayer can record audio/screenshots onto the mined card, and it
	// records from its ACTIVE tab.
	const mediaNote = $derived.by(() => {
		if ($playerStatus.mode !== 'asbplayer' || $playerStatus.ws_clients === 0)
			return ' — no audio/screenshot without asbplayer';
		if ($asbContext.loaded_from_asbplayer && !$asbContext.loaded_is_active)
			return " — ⚠ asbplayer's active tab is not the loaded video";
		return '';
	});
	const selectableKeys = $derived(terms.filter((t) => !isMined(t)).map(termKey));
	const allSelected = $derived(
		selectableKeys.length > 0 && selectableKeys.every((k) => $selectedTerms.has(k))
	);
	const someSelected = $derived(selectableKeys.some((k) => $selectedTerms.has(k)));

	function rowClick(e: MouseEvent, term: Term) {
		// Rows see the popover-dismissing click before the popover's own
		// window listener closes it — that click must not toggle selection.
		if (defPopover) return;
		if (!canMine || isMined(term)) return;
		if (e.ctrlKey || e.metaKey) return;
		// Only empty row space toggles — not cell content (copyable text, buttons).
		// `.sentence`/`.meta` also match SentenceView's full-width blocks.
		const target = e.target as HTMLElement;
		if (
			target !== e.currentTarget &&
			!target.matches('.sel, .term-cell, .jlpt-cell, .sentence, .meta')
		)
			return;
		if (window.getSelection()?.toString()) return;
		toggleSelected(termKey(term));
	}

	let batchEntries = $state<BatchEntry[] | null>(null);

	// Selections survive filter changes, so the batch must draw from ALL terms —
	// `terms` is only the filtered view and would silently drop hidden picks.
	const hiddenSelected = $derived(
		$selectedTerms.size - terms.filter((t) => $selectedTerms.has(termKey(t))).length
	);

	let showQueueDetails = $state(false);
	const queueDetails = $derived.by(() => {
		const visible = new Set(terms.map(termKey));
		return ($fileResult?.terms ?? terms)
			.filter((t) => $selectedTerms.has(termKey(t)))
			.map((t) => {
				const key = termKey(t);
				const opt = $queuedMineOptions[key];
				return {
					key,
					lemma: t.lemma_form,
					hidden: !visible.has(key),
					formatName: opt?.formatName,
					entryIndex: opt?.entryIndex
				};
			});
	});
	$effect(() => {
		if ($selectedTerms.size === 0 || $mineQueueState !== null) showQueueDetails = false;
	});

	$effect(() => {
		if ($mineQueueState?.key === undefined) return;
		requestAnimationFrame(() => {
			document.querySelector('.row.mining')?.scrollIntoView({ block: 'center', behavior: 'smooth' });
		});
	});

	function startBatch() {
		const entries: BatchEntry[] = ($fileResult?.terms ?? terms)
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
					return [
						{
							idx,
							sentence: o.sentence.text,
							timestamp: o.sentence.timestamp,
							surface: termHighlightText(t, o)
						}
					];
				});
				return {
					term: t,
					key,
					surface: occ ? termHighlightText(t, occ) : t.surface_form,
					sentence: occ?.sentence.text ?? '',
					timestamp: occ?.sentence.timestamp ?? null,
					entryIndex: $queuedMineOptions[key]?.entryIndex,
					formatName: $queuedMineOptions[key]?.formatName,
					explicit: occIdx[key] !== undefined,
					alternatives
				};
			});
		const keys = entries.map((e) => normalizeSentence(e.sentence)).filter((s) => s !== '');
		if (new Set(keys).size === keys.length) {
			void mineQueue(
				entries.map(({ term, surface, sentence, timestamp, entryIndex, formatName }) => ({
					term,
					surface,
					sentence,
					timestamp,
					entryIndex,
					formatName
				}))
			);
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
		oncancel={() => {
			// Layered dismissal: Escape/backdrop peels the popover first.
			if (defPopover) {
				defPopover = null;
				return;
			}
			batchEntries = null;
		}}
		onlookup={(req) => $yomitanReachable && (defPopover = { ...req, mineable: null })}
		onhover={(fn) => (hovered = fn)}
	/>
{/if}

{#if confirmMine}
	<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions --
	     Escape closes it via the window handler below. -->
	<div class="backdrop" onclick={() => (confirmMine = null)}>
		<div
			class="dialog"
			role="dialog"
			aria-modal="true"
			aria-label="Mine individually"
			tabindex="-1"
			onclick={(e) => e.stopPropagation()}
		>
			<p class="dialog-body">
				You have {$selectedTerms.size} term{$selectedTerms.size === 1 ? '' : 's'} selected for batch
				mining. Mine 「<span lang="ja">{confirmMine.term.lemma_form}</span>」 individually now?
			</p>
			<footer class="dialog-footer">
				<button class="bulk-btn primary" onclick={confirmedMine}>Mine individually</button>
				<button class="bulk-btn" onclick={() => (confirmMine = null)}>Cancel</button>
			</footer>
		</div>
	</div>
{/if}

{#if $mineQueueState}
	<div class="bulk-bar">
		<span class="bulk-info">
			Mining {$mineQueueState.done + 1}/{$mineQueueState.total} 「{$mineQueueState.current}」
		</span>
		<button class="bulk-btn" onclick={cancelQueue}>Cancel</button>
	</div>
{:else if canMine && $selectedTerms.size > 0}
	{#if showQueueDetails}
		<div class="bulk-details">
			<div class="detail-row detail-head">
				<span>Term</span>
				<span>Card format</span>
			</div>
			{#each queueDetails as d (d.key)}
				<div class="detail-row">
					<span lang="ja">
						{d.lemma}{#if d.hidden}<span class="detail-dim"> (hidden)</span>{/if}
					</span>
					<span>
						{#if $cardFormats.length > 1}
							<select
								class="detail-select"
								value={d.formatName ?? $cardFormats[0].name}
								aria-label={`Card format for ${d.lemma}`}
								onchange={(e) => setQueuedFormat(d.key, e.currentTarget.value)}
							>
								{#each $cardFormats as f (f.name)}
									<option value={f.name}>{f.name}</option>
								{/each}
							</select>
						{:else}
							{$cardFormats[0]?.name ?? '—'}
						{/if}
						{#if d.entryIndex !== undefined}<span class="detail-dim">· def #{d.entryIndex + 1}</span
							>{/if}
					</span>
				</div>
			{/each}
		</div>
	{/if}
	<div class="bulk-bar">
		<span class="bulk-info">
			{$selectedTerms.size} selected{hiddenSelected > 0
				? ` · ${hiddenSelected} hidden by filters`
				: ''}
		</span>
		<button
			class="bulk-btn"
			title="Show the queued terms and the card format each will use"
			onclick={() => (showQueueDetails = !showQueueDetails)}
			>Details {showQueueDetails ? '▾' : '▸'}</button
		>
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

{#if editColumns}
	<div class="col-edit-bar">
		{#each editCols as col (col.id)}
			<!-- svelte-ignore a11y_no_static_element_interactions -- pointer drag;
			     the checkbox stays keyboard-accessible. -->
			<span
				class="col-edit"
				class:col-hidden={!col.visible}
				class:dragging={dragId === col.id}
				data-col={col.id}
				onpointerdown={(e) => pillDown(e, col.id)}
				onpointermove={pillMove}
				onpointerup={pillUp}
			>
				<input
					type="checkbox"
					bind:checked={col.visible}
					disabled={col.id === 'term'}
					onchange={commitColumns}
					aria-label={`Show ${COLUMN_LABELS[col.id]} column`}
				/>
				{COLUMN_LABELS[col.id]}
			</span>
		{/each}
		<span class="col-edit-hint">drag to reorder · untick to hide · saves as you go</span>
		<button class="bulk-btn" onclick={() => (editColumns = false)}>Done</button>
	</div>
{/if}

<div class="table" style="grid-template-columns: {gridTemplate}">
	<!-- svelte-ignore a11y_no_static_element_interactions -- right-click column
	     editing is a mouse affordance; keyboard browsing is issue #91. -->
	<div class="row head" oncontextmenu={openHeaderMenu}>
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
		{#each renderCols as id (id)}
			{#if id === 'term'}
				<span>Term</span>
			{:else if id === 'jlpt'}
				<span class="jlpt-cell head-cell">
					<button
						class="head-btn"
						class:active={jlptActive}
						title={jlptActive ? sortedTip('JLPT') : 'Sort by JLPT'}
						onclick={clickJlpt}
					>
						JLPT
						{#if jlptActive}
							<span class="arrow active">{dirArrow($tableSort.dir)}</span>
						{:else}
							<span class="arrow hint">⇅</span>
							<span class="arrow preview">{dirArrow(defaultDir('jlpt'))}</span>
						{/if}
					</button>
				</span>
			{:else if id === 'sentence'}
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
			{:else if id === 'frequency'}
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
			{:else if id === 'pos'}
				<span>POS</span>
			{/if}
		{/each}
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
			class:selectable={canMine && !isMined(term)}
			class:selected={canMine && $selectedTerms.has(key)}
			class:mining={$mineQueueState?.key === key}
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
			{#each renderCols as id (id)}
				{#if id === 'term'}
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
						{:else if canMine}
							<button
								class="chip mine"
								disabled={$miningTerm !== null || $playerBusy}
								title={$playerBusy && $miningTerm === null
									? 'Waiting for asbplayer to finish recording the mined line…'
									: 'Create an Anki card from the displayed sentence' + mediaNote}
								onclick={() => mineClicked(term, occs)}
							>
								{#if $miningTerm === term.lemma_form}…{:else}
									<svg
										viewBox="0 0 24 24"
										width="1em"
										height="1em"
										fill="none"
										stroke="currentColor"
										stroke-width="2.4"
										stroke-linecap="round"
										aria-hidden="true"
									>
										<path d="M3 21 L13.5 10.5" />
										<path d="M10 4 Q 17.8 6.2 20 14" />
									</svg>
								{/if}
							</button>
						{/if}
					</span>
				{:else if id === 'jlpt'}
					<span class="jlpt-cell">
						{#if term.jlpt_level}
							<span class="jlpt-chip">{term.jlpt_level}</span>
						{/if}
					</span>
				{:else if id === 'sentence'}
					<div class="sentence">
						{#if occs.length > 0}
							<SentenceView
								occurrences={occs}
								{term}
								bind:currentIndex={occIdx[key]}
								onlookup={segmentLookup}
								onhover={(fn) => (hovered = fn)}
							/>
						{:else}
							<span class="empty">—</span>
						{/if}
					</div>
				{:else if id === 'frequency'}
					<span class="num">{freqLabel(term)}</span>
				{:else if id === 'pos'}
					<span class="pos" style="color: {posColor(term.part_of_speech)}">
						{posLabels[term.part_of_speech] ?? term.part_of_speech}
					</span>
				{/if}
			{/each}
		</div>
	{/each}
</div>

{#if defPopover}
	{@const mineable = defPopover.mineable}
	<DefinitionPopover
		text={defPopover.text}
		label={defPopover.label}
		anchor={defPopover.anchor}
		scale={$settings?.definition_scale ?? 1}
		showMine={canMine && mineable !== null && !isMined(mineable.term)}
		mineDisabled={$miningTerm !== null || $playerBusy || $selectedTerms.size > 0}
		mineTitle={$selectedTerms.size > 0
			? 'A batch selection is active — Queue this term instead, or clear the selection'
			: 'Create an Anki card from the displayed sentence' + mediaNote}
		formats={$cardFormats}
		onmine={(entryIndex, formatName) =>
			mineable && mine(mineable.term, mineable.occs, entryIndex, formatName)}
		onqueue={(entryIndex, formatName) =>
			mineable && queueWithEntry(termKey(mineable.term), entryIndex, formatName)}
		onclose={() => (defPopover = null)}
	/>
{/if}

<svelte:window
	onkeydown={trackMods}
	onkeyup={trackMods}
	onmousemove={(e) => (ctrlHeld = e.ctrlKey || e.metaKey)}
	onblur={() => (ctrlHeld = false)}
/>

<style>
	/* One shared track list (rows subgrid it) so the max-content term column is
	   sized globally — per-row grids each size their own and misalign. The
	   template itself is inline (built from the column config, issue #122). */
	.table {
		display: grid;
		/* Subgrid rows inherit this; a gap on .row would be ignored. */
		column-gap: 0.75rem;
		font-variant-numeric: tabular-nums;
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
	.row.selectable {
		cursor: pointer;
	}
	/* The row the batch queue is currently mining. */
	.row.mining {
		outline: 2px solid var(--cyan);
		outline-offset: -2px;
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
		cursor: text;
	}
	/* The furigana annotation only adds height ABOVE the base text; pad the same
	   amount below (rt is 0.5em at line-height 1) so row-centering keeps the base
	   text centered instead of pushing it down. */
	.term :global(.word:has(rt)) {
		padding-bottom: 0.5em;
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
	/* Queue-details panel, stacked just above the fixed bulk-bar. */
	.bulk-details {
		position: fixed;
		bottom: 4rem;
		left: 50%;
		transform: translateX(-50%);
		z-index: 40;
		min-width: 18rem;
		max-width: 90vw;
		max-height: 40vh;
		overflow-y: auto;
		padding: 0.45rem 0.9rem;
		background: var(--bg-dark);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
		font-size: 0.85rem;
	}
	.detail-row {
		display: grid;
		grid-template-columns: minmax(6rem, auto) 1fr;
		gap: 1rem;
		padding: 0.15rem 0;
	}
	.detail-head {
		color: var(--comment);
		font-size: 0.75rem;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		border-bottom: 1px solid var(--border);
		padding-bottom: 0.3rem;
		margin-bottom: 0.2rem;
	}
	.detail-dim {
		color: var(--comment);
		font-size: 0.8em;
	}
	.detail-select {
		max-width: 100%;
		font-size: 0.8rem;
	}
	.backdrop {
		position: fixed;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--bg-darker) 70%, transparent);
		z-index: 50;
	}
	.dialog {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		width: min(420px, 92%);
		padding: 1rem;
		background: var(--bg-dark);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
	}
	.dialog-body {
		margin: 0;
		font-size: 0.9rem;
	}
	.dialog-footer {
		display: flex;
		gap: 0.5rem;
		justify-content: flex-end;
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
	.col-edit-bar {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.4rem;
		margin-bottom: 0.35rem;
		padding: 0.35rem 0.6rem;
		background: var(--bg-light);
		border: 1px solid var(--border);
		border-radius: var(--radius);
	}
	.col-edit {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.15rem 0.5rem;
		font-size: 0.8rem;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		color: var(--fg);
		background: var(--bg-dark);
		border: 1px dashed var(--border);
		border-radius: var(--radius);
		cursor: grab;
		white-space: nowrap;
		user-select: none;
		touch-action: none;
	}
	.col-edit.dragging {
		cursor: grabbing;
		border-style: solid;
		border-color: var(--cyan);
	}
	.col-edit.col-hidden {
		opacity: 0.45;
	}
	.col-edit-hint {
		margin-left: auto;
		font-size: 0.8rem;
		color: var(--comment);
	}
</style>
