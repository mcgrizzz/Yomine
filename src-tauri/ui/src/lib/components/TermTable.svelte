<script lang="ts">
	import type { DefinitionEntry, SentenceDto, Term, TimeStampDto } from '$lib/ipc';
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
		adhocQueue,
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
		addedKeys,
		minedKeys,
		minedNoteIds,
		minedTerms,
		miningTerm,
		normalizeSentence,
		openInAnki,
		pinOccurrence,
		playerBusy,
		playerStatus,
		posCatalog,
		queuedCount,
		queuedMineOptions,
		queueAdhoc,
		queueWithEntry,
		retryMedia,
		selectedTerms,
		setSelected,
		setTableColumns,
		settings,
		tableSearch,
		tableSort,
		toggleIgnore,
		toggleSelected,
		yomitanReachable,
		type OccurrencePin,
		type QueueItem
	} from '$lib/stores';
	import { untrack } from 'svelte';
	import { Menu } from '@tauri-apps/api/menu';
	import { furiganaText } from '$lib/furigana';
	import DefinitionPopover from './DefinitionPopover.svelte';
	import Furigana from './Furigana.svelte';
	import MiningQueueModal from './MiningQueueModal.svelte';
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
		/** The hovered span, so an entry with no table Term is still mineable. */
		segment: { sentence: SentenceDto; surface: string } | null;
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
				mineable: { term, occs: occurrencesOf(term) },
				segment: null
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
		defPopover = {
			text: req.text,
			label: req.label,
			anchor: req.anchor,
			mineable,
			segment: { sentence: req.sentence, surface: req.seg.surface }
		};
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

	// Each row's displayed occurrence - a queued term mines its pin instead.
	let occIdx = $state<Record<string, number>>({});

	let userChosen = $state<Record<string, boolean>>({});

	const pinFor = (key: string): OccurrencePin => ({
		occIdx: occIdx[key] ?? 0,
		userChosen: userChosen[key] ?? false
	});

	function navigated(key: string, index: number) {
		userChosen[key] = true;
		if ($selectedTerms.has(key)) pinOccurrence(key, { occIdx: index, userChosen: true });
	}

	// Must precede the search effect: on load this clears, then that repopulates.
	let lastFile: unknown = null;
	$effect(() => {
		const file = $fileResult;
		if (file === lastFile) return;
		lastFile = file;
		untrack(() => {
			occIdx = {};
			userChosen = {};
		});
	});

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
		if ($queuedCount > 0) confirmMine = { term, occs };
		else mine(term, occs);
	}

	function confirmedMine() {
		if (!confirmMine) return;
		const { term, occs } = confirmMine;
		confirmMine = null;
		mine(term, occs);
	}

	// asbplayer enrichment needs asbplayer active (same rule as seeking) + a cue.
	const viaFor = (ts: TimeStampDto | null): 'asbplayer' | 'direct' =>
		$playerStatus.mode === 'asbplayer' && $playerStatus.ws_clients > 0 && ts !== null
			? 'asbplayer'
			: 'direct';

	function mine(
		term: Term,
		occs: Occurrence[],
		entryIndex?: number,
		formatName?: string,
		scanText?: string
	) {
		const occ = occs[Math.min(occIdx[termKey(term)] ?? 0, occs.length - 1)];
		const ts = occ?.sentence.timestamp ?? null;
		const surface = occ ? termHighlightText(term, occ) : term.surface_form;
		void mineTerm(
			term.lemma_form,
			occ?.sentence.text ?? '',
			ts,
			viaFor(ts),
			surface,
			entryIndex,
			formatName,
			scanText
		);
	}

	/** Only the row whose term IS this entry — `termCoversSegment` merely overlaps. */
	function rowFor(entry: DefinitionEntry): { term: Term; occs: Occurrence[] } | null {
		const p = defPopover;
		if (!p?.mineable) return null;
		if (!p.segment) return p.mineable;
		const t = p.mineable.term;
		return entry.expression === t.lemma_form ||
			entry.expression === t.surface_form ||
			entry.expression === t.full_segment
			? p.mineable
			: null;
	}

	function queueable(entry: DefinitionEntry): boolean {
		const row = rowFor(entry);
		return row ? !isMined(row.term) : defPopover?.segment != null;
	}

	/** Mine a hovered span that no table row represents. */
	function mineSegment(
		segment: { sentence: SentenceDto; surface: string },
		entry: DefinitionEntry,
		formatName?: string,
		scanText?: string
	) {
		const ts = segment.sentence.timestamp ?? null;
		void mineTerm(
			entry.expression,
			segment.sentence.text,
			ts,
			viaFor(ts),
			segment.surface,
			entry.index,
			formatName,
			scanText
		);
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
		if ($asbContext.loaded_from_asbplayer && !$asbContext.loaded_has_subtitles)
			return ' — the loaded video has no subtitles in asbplayer; card will get no audio/screenshot';
		if ($asbContext.loaded_from_asbplayer && !$asbContext.loaded_is_active)
			return " — ⚠ the video's tab isn't active; screenshots capture the visible tab";
		// Timestamp-less sources (EPUB/TXT) never enrich, so no target note.
		const subtitleFile =
			$fileResult?.source_file.file_type === 'SRT' ||
			$fileResult?.source_file.file_type === 'SSA';
		if (!$asbContext.loaded_from_asbplayer && subtitleFile)
			return " — captures media from asbplayer's active tab";
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
		const key = termKey(term);
		toggleSelected(key, pinFor(key));
	}

	let batchEntries = $state<BatchEntry[] | null>(null);

	// Selections survive filter changes, so the batch must draw from ALL terms —
	// `terms` is only the filtered view and would silently drop hidden picks.
	const hiddenSelected = $derived(
		$selectedTerms.size - terms.filter((t) => $selectedTerms.has(termKey(t))).length
	);

	let showQueueDetails = $state(false);
	$effect(() => {
		if ($queuedCount === 0 || $mineQueueState !== null) showQueueDetails = false;
	});

	$effect(() => {
		if ($mineQueueState?.key === undefined) return;
		requestAnimationFrame(() => {
			document.querySelector('.row.mining')?.scrollIntoView({ block: 'center', behavior: 'smooth' });
		});
	});

	function startBatch() {
		const rows: BatchEntry[] = ($fileResult?.terms ?? terms)
			.filter((t) => $selectedTerms.has(termKey(t)) && !isMined(t))
			.map((t) => {
				const key = termKey(t);
				const occs = occurrencesOf(t);
				const pinned = $queuedMineOptions[key]?.occIdx ?? occIdx[key] ?? 0;
				const occ = occs[Math.min(pinned, occs.length - 1)];
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
					lemma: t.lemma_form,
					key,
					surface: occ ? termHighlightText(t, occ) : t.surface_form,
					sentence: occ?.sentence.text ?? '',
					timestamp: occ?.sentence.timestamp ?? null,
					entryIndex: $queuedMineOptions[key]?.entryIndex,
					formatName: $queuedMineOptions[key]?.formatName,
					scanText: $queuedMineOptions[key]?.scanText,
					explicit: $queuedMineOptions[key]?.userChosen ?? false,
					alternatives
				};
			});
		// explicit + no alternatives: the conflict modal keeps their sentence and moves the row.
		const adhoc: BatchEntry[] = $adhocQueue.map((a) => ({
			lemma: a.lemma,
			key: a.key,
			surface: a.surface,
			sentence: a.sentence,
			timestamp: a.timestamp,
			entryIndex: a.entryIndex,
			formatName: a.formatName,
			scanText: a.scanText,
			explicit: true,
			alternatives: []
		}));
		const entries = [...rows, ...adhoc];
		const keys = entries.map((e) => normalizeSentence(e.sentence)).filter((s) => s !== '');
		if (new Set(keys).size === keys.length) {
			void mineQueue(
				entries.map(
					({ lemma, key, surface, sentence, timestamp, entryIndex, formatName, scanText }) => ({
						lemma,
						key,
						surface,
						sentence,
						timestamp,
						entryIndex,
						formatName,
						scanText
					})
				)
			);
			return;
		}
		batchEntries = entries;
	}

	function conflictsResolved(items: QueueItem[], occIdxPatch: Record<string, number>) {
		// Sync the rows to any reassigned occurrences so display = mined.
		for (const [key, idx] of Object.entries(occIdxPatch)) {
			occIdx[key] = idx;
			// Not `true`: auto-swap reassigns without the user choosing.
			pinOccurrence(key, { occIdx: idx, userChosen: userChosen[key] ?? false });
		}
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
		onlookup={(req) =>
			$yomitanReachable && (defPopover = { ...req, mineable: null, segment: null })}
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
				You have {$queuedCount} term{$queuedCount === 1 ? '' : 's'} selected for batch
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
{:else if canMine && $queuedCount > 0}
	{#if showQueueDetails}
		<MiningQueueModal {terms} onclose={() => (showQueueDetails = false)} />
	{/if}
	<div class="bulk-bar">
		<span class="bulk-info">
			{$queuedCount} selected{hiddenSelected > 0
				? ` · ${hiddenSelected} hidden by filters`
				: ''}
		</span>
		<button
			class="bulk-btn"
			title="Review the queued terms — entry, card format, and what to drop"
			onclick={() => (showQueueDetails = true)}>Details…</button
		>
		{#if canMine}
			<button
				class="bulk-btn primary"
				disabled={$miningTerm !== null || $playerBusy}
				title="Mine the selected terms one by one, in timestamp order"
				onclick={startBatch}>Mine {$queuedCount}</button
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
					onchange={() => setSelected(selectableKeys, !allSelected, pinFor)}
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
						onchange={() => toggleSelected(key, pinFor(key))}
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
								onnavigate={(idx) => navigated(key, idx)}
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
					<span class="pos">
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
		canMine={canMine && (mineable !== null || defPopover.segment !== null)}
		canQueue={queueable}
		isDuplicate={(entry) => entry.known || $minedKeys.has(entry.key) || $addedKeys.has(entry.key)}
		mineDisabled={(entry) =>
			$miningTerm !== null || $playerBusy || ($queuedCount > 0 && queueable(entry))}
		mineTitle={(entry) =>
			$queuedCount > 0 && queueable(entry)
				? 'A batch selection is active — Queue this term instead, or clear the selection'
				: 'Create an Anki card from the displayed sentence' + mediaNote}
		formats={$cardFormats}
		onmine={(entry, formatName) => {
			const row = rowFor(entry);
			if (row) mine(row.term, row.occs, entry.index, formatName, defPopover?.text);
			else if (defPopover?.segment)
				mineSegment(defPopover.segment, entry, formatName, defPopover.text);
		}}
		onqueue={(entry, formatName) => {
			const row = rowFor(entry);
			if (row) {
				const key = termKey(row.term);
				queueWithEntry(key, entry.index, formatName, defPopover?.text, pinFor(key));
			} else if (defPopover?.segment) {
				queueAdhoc({
					key: `adhoc:${defPopover.segment.sentence.id}:${entry.expression}`,
					lemma: entry.expression,
					surface: defPopover.segment.surface,
					sentence: defPopover.segment.sentence.text,
					timestamp: defPopover.segment.sentence.timestamp ?? null,
					entryIndex: entry.index,
					formatName,
					scanText: defPopover.text
				});
			}
		}}
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
	/* Box-centred, not baseline-centred: an inline child rides ~1px low on the strut. */
	.sel,
	.jlpt-cell:not(.head-cell) {
		display: inline-flex;
		align-items: center;
		justify-content: center;
	}
	.sel input {
		cursor: pointer;
	}
	.row:not(.head):hover {
		background: var(--bg-raised);
	}
	.row.selected {
		background: color-mix(in srgb, var(--accent) 7%, transparent);
	}
	.row.selected:hover {
		background: color-mix(in srgb, var(--accent) 12%, transparent);
	}
	.row.selectable {
		cursor: pointer;
	}
	/* The row the batch queue is currently mining. */
	.row.mining {
		outline: 2px dashed var(--accent);
		outline-offset: -2px;
	}
	.row.head {
		position: sticky;
		top: 0;
		z-index: var(--z-sticky);
		background: var(--bg-panel);
		color: var(--text-muted);
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
		border-radius: var(--radius-sm);
		color: inherit;
		font: inherit;
		text-transform: inherit;
		letter-spacing: inherit;
		cursor: pointer;
	}
	/* Active-column highlight. */
	.head-btn.active {
		background: color-mix(in srgb, var(--accent) 10%, transparent);
		color: var(--text);
	}
	.head-btn:hover {
		background: var(--bg-raised);
		color: var(--text);
	}
	.arrow.active {
		color: var(--accent);
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
		color: var(--text-muted);
		font-size: 0.7rem;
		text-transform: none;
		letter-spacing: normal;
		cursor: pointer;
		white-space: nowrap;
	}
	.mode:hover {
		color: var(--text);
	}
	.num {
		text-align: right;
	}
	.term-cell {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
	}
	.term {
		font-size: 1.5rem;
		color: var(--term);
		line-height: 1.1;
		cursor: text;
		/* The CJK ideographic em box (sTypo 880/-120) centres 0.38em above the baseline,
		   0.056em below this line box's centre; pad twice that to cancel it. */
		padding-bottom: 0.112em;
	}
	/* The furigana annotation only adds height ABOVE the base text; pad the same
	   amount below (rt is 0.5em at line-height 1) so row-centering keeps the base
	   text centered instead of pushing it down. */
	.term :global(.word:has(rt)) {
		padding-bottom: 0.5em;
	}
	/* Kept above .ignored so an ignored term still greys out. */
	.term.mined-term {
		color: var(--success);
	}
	/* Every chip state shares one footprint so a completed mine doesn't shift the row. */
	.chip {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 1.75rem;
		height: 1.75rem;
		padding: 0;
		font-size: 0.95rem;
		line-height: 1;
		border-radius: var(--radius);
	}
	.mine {
		color: var(--accent);
		background: var(--bg-raised);
		border: 1px solid var(--border);
		cursor: pointer;
	}
	.mine:hover:not(:disabled) {
		background: var(--bg-hover);
		border-color: var(--accent);
	}
	.mine:disabled {
		opacity: 0.5;
		cursor: default;
	}
	.mined {
		color: var(--success);
		background: color-mix(in srgb, var(--success) 12%, transparent);
		cursor: help;
	}
	.mined.openable {
		border: 1px solid color-mix(in srgb, var(--success) 35%, transparent);
		cursor: pointer;
	}
	.mined.openable:hover {
		background: color-mix(in srgb, var(--success) 25%, transparent);
	}
	/* Note exists but asbplayer media never landed — click retries the enrichment. */
	.warn {
		color: var(--warning);
		background: color-mix(in srgb, var(--warning) 12%, transparent);
		border: 1px solid color-mix(in srgb, var(--warning) 35%, transparent);
		cursor: pointer;
	}
	.warn:hover:not(:disabled) {
		background: color-mix(in srgb, var(--warning) 25%, transparent);
	}
	.warn:disabled {
		opacity: 0.5;
		cursor: default;
	}
	/* Ignored-in-place: greyed until the next refresh drops the row. */
	.term.ignored {
		color: var(--text-muted);
	}
	/* Pointing-hand while Ctrl/Cmd is held (the click-to-ignore affordance). */
	.term.ignorable {
		cursor: pointer;
	}
	.pos {
		font-size: 0.9rem;
		color: var(--text-muted);
	}
	.jlpt-chip {
		padding: 0.05rem 0.3rem;
		font-size: 0.7rem;
		color: var(--text-muted);
		background: var(--bg-raised);
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
		z-index: var(--z-bar);
		display: flex;
		align-items: center;
		gap: 0.6rem;
		max-width: 90vw;
		padding: 0.45rem 0.9rem;
		background: var(--bg-panel);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		box-shadow: var(--shadow-overlay);
		font-size: 0.85rem;
	}
	.bulk-info {
		color: var(--text);
	}
	.bulk-btn {
		padding: 0.25rem 0.6rem;
	}
	.bulk-btn:disabled {
		opacity: 0.5;
		cursor: default;
	}
	.backdrop {
		position: fixed;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--bg-deep) 70%, transparent);
		z-index: var(--z-modal);
	}
	.dialog {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		width: min(420px, 92%);
		padding: 1rem;
		background: var(--bg-panel);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		box-shadow: var(--shadow-modal);
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
		color: var(--text-muted);
	}
	.no-match {
		grid-column: 1 / -1;
		margin: 0;
		padding: 1.5rem 0.5rem;
		color: var(--text-muted);
		text-align: center;
	}
	.col-edit-bar {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.4rem;
		margin-bottom: 0.35rem;
		padding: 0.35rem 0.6rem;
		background: var(--bg-raised);
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
		color: var(--text);
		background: var(--bg-panel);
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
		border-color: var(--accent);
	}
	.col-edit.col-hidden {
		opacity: 0.45;
	}
	.col-edit-hint {
		margin-left: auto;
		font-size: 0.8rem;
		color: var(--text-muted);
	}
</style>
