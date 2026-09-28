<script lang="ts">
	import { tick as settled, untrack } from 'svelte';
	import {
		DEFAULT_HORIZON,
		frequencyPoints,
		POS_ALIASES,
		scoreParts,
		type MissReason,
		type ScoreParts
	} from '$lib/autopick';
	import { cachedEntries, entryLabel, fetchEntries, glossarySummary } from '$lib/definitions';
	import {
		getDefaultEntry,
		getLineMedia,
		prepareMedia,
		type DefinitionEntry,
		type SentenceDto,
		type Term,
		type TimeStampDto
	} from '$lib/ipc';
	import {
		addReviewPick,
		adhocQueue,
		autoReview,
		cardFormats,
		dropAdhoc,
		fileResult,
		knowledge,
		localVideo,
		miningMode,
		pinOccurrence,
		playerConnected,
		posCatalog,
		queueAdhoc,
		queuedCount,
		queuedMineOptions,
		selectedTerms,
		setAdhocEntry,
		setAdhocFormat,
		setQueuedEntry,
		setQueuedFormat,
		seekTimestamp,
		setSelected,
		settings,
		skipReview,
		type AdhocQueued,
		type QueuedMineOption
	} from '$lib/stores';
	import { harmonic, occurrencesOf, termKey } from '$lib/table';
	import DefinitionPopover from './DefinitionPopover.svelte';
	import Furigana from './Furigana.svelte';
	import Glossary from './Glossary.svelte';
	import Modal from './Modal.svelte';
	import { termCoversSegment, termHighlightText, type Occurrence } from './SentenceView.svelte';

	let {
		canMine,
		onmine,
		onclose
	}: { canMine: boolean; onmine: () => void; onclose: () => void } = $props();

	interface Row {
		key: string;
		lemma: string;
		reading: string;
		term: Term | null;
		occs: Occurrence[];
		occIdx: number;
		sentence: SentenceDto | null;
		text: string;
		surface: string;
		entryIndex?: number;
		scanText?: string;
		formatName?: string;
		/** Null for entries not in the table and for words no frequency list ranks. */
		score: ScoreParts | null;
		ticked: boolean;
	}

	/** Unticked rows stay listed; ticking one restores its settings. */
	let dropped = $state<Map<string, { row: Row; option?: QueuedMineOption; adhoc?: AdhocQueued }>>(
		new Map()
	);

	const review = $derived($autoReview);
	const horizon = $derived($knowledge?.horizon ?? DEFAULT_HORIZON);
	const prefs = $derived($settings?.auto_mine);
	const posLabel = (pos: string) =>
		$posCatalog.find((p) => p.key === (POS_ALIASES[pos] ?? pos))?.display_name ?? pos;
	const total = (s: ScoreParts) => Math.round(s.frequency + s.wordType + s.jlpt);
	const signed = (n: number) => (n < 0 ? `−${Math.abs(Math.round(n))}` : `+${Math.round(n)}`);

	function scoreOf(t: Term): ScoreParts | null {
		const rank = harmonic(t);
		if (!prefs || rank === Infinity) return null;
		return scoreParts(rank, t.part_of_speech, t.jlpt_level, horizon, prefs);
	}

	function termRow(t: Term, ticked: boolean): Row {
		const key = termKey(t);
		const option = $queuedMineOptions[key];
		const occs = occurrencesOf(t, $fileResult?.sentences ?? []);
		const occIdx = Math.min(option?.occIdx ?? 0, Math.max(occs.length - 1, 0));
		const occ = occs[occIdx];
		return {
			key,
			lemma: t.lemma_form,
			reading: t.lemma_reading,
			term: t,
			occs,
			occIdx,
			sentence: occ?.sentence ?? null,
			text: occ?.sentence.text ?? '',
			surface: occ ? termHighlightText(t, occ) : t.surface_form,
			entryIndex: option?.entryIndex,
			scanText: option?.scanText,
			formatName: option?.formatName,
			score: scoreOf(t),
			ticked
		};
	}

	const rows = $derived.by(() => {
		const terms = $fileResult?.terms ?? [];
		const live = terms.filter((t) => $selectedTerms.has(termKey(t))).map((t) => termRow(t, true));
		const adhoc: Row[] = $adhocQueue.map((a) => ({
			key: a.key,
			lemma: a.lemma,
			reading: '',
			term: null,
			occs: [],
			occIdx: 0,
			sentence: null,
			text: a.sentence,
			surface: a.surface,
			entryIndex: a.entryIndex,
			scanText: a.scanText,
			formatName: a.formatName,
			score: null,
			ticked: true
		}));
		const shown = new Set([...live, ...adhoc].map((r) => r.key));
		const unticked = [...dropped.values()]
			.filter((d) => !shown.has(d.row.key))
			.map((d) => ({ ...d.row, ticked: false }));
		const all = [...live, ...adhoc, ...unticked];
		// Auto mode's picks read best first; a selection reads in the order it plays.
		return review
			? all.sort((a, b) => (b.score ? total(b.score) : -Infinity) - (a.score ? total(a.score) : -Infinity))
			: all.sort(
					(a, b) =>
						(a.sentence?.timestamp?.start_secs ?? Infinity) -
						(b.sentence?.timestamp?.start_secs ?? Infinity)
				);
	});

	const next = $derived(review?.next.filter((n) => !$selectedTerms.has(termKey(n.term))) ?? []);

	const MISSED: Record<MissReason, string> = {
		limit: 'Past the card limit',
		min_score: 'Under the minimum score',
		sentence_used: 'Its sentences are taken by other picks'
	};

	let focusedKey = $state<string | null>(null);
	const focused = $derived(rows.find((r) => r.key === focusedKey) ?? rows[0] ?? null);

	function untick(row: Row) {
		const adhoc = $adhocQueue.find((a) => a.key === row.key);
		dropped = new Map(dropped).set(row.key, { row, option: $queuedMineOptions[row.key], adhoc });
		if (adhoc) dropAdhoc(row.key);
		else setSelected([row.key], false);
	}

	function tick(row: Row) {
		const saved = dropped.get(row.key);
		if (saved?.adhoc) queueAdhoc(saved.adhoc);
		else {
			setSelected([row.key], true);
			if (saved?.option) queuedMineOptions.update((m) => ({ ...m, [row.key]: saved.option! }));
		}
		const rest = new Map(dropped);
		rest.delete(row.key);
		dropped = rest;
	}

	function addNext(t: Term) {
		const key = termKey(t);
		const occs = occurrencesOf(t, $fileResult?.sentences ?? []);
		const best = occs.reduce(
			(b, o, i) => (o.sentence.comprehension > occs[b].sentence.comprehension ? i : b),
			0
		);
		setSelected([key], true, () => ({ occIdx: best, userChosen: false }));
		addReviewPick(key);
		focusedKey = key;
	}

	/** Segments of the row's sentence, with the mined word marked. */
	function sentenceParts(row: Row): { text: string; hit: boolean }[] {
		const occ = row.occs[row.occIdx];
		if (row.term && occ) {
			const term = row.term;
			return occ.sentence.segments.map((seg) => ({
				text: seg.surface,
				hit: termCoversSegment(term, occ.start, seg)
			}));
		}
		const at = row.surface ? row.text.indexOf(row.surface) : -1;
		if (at < 0) return [{ text: row.text, hit: false }];
		return [
			{ text: row.text.slice(0, at), hit: false },
			{ text: row.surface, hit: true },
			{ text: row.text.slice(at + row.surface.length), hit: false }
		];
	}

	// Yomitan entries by scan text, and each row's default entry.
	let entries = $state<Record<string, DefinitionEntry[]>>({});
	let defaults = $state<Record<string, number>>({});
	const requested = new Set<string>();
	const scanOf = (row: Row) => row.scanText ?? row.lemma;

	$effect(() => {
		for (const row of rows) {
			const text = scanOf(row);
			untrack(() => {
				if (!requested.has(text)) {
					requested.add(text);
					const hit = cachedEntries(text);
					if (hit) entries[text] = hit;
					else
						fetchEntries(text).then(
							(list) => (entries[text] = list),
							() => (entries[text] = [])
						);
				}
				if (row.term && !(row.key in defaults)) {
					defaults[row.key] = -1;
					getDefaultEntry(row.key, row.lemma, row.scanText ?? null).then(
						(i) => (defaults[row.key] = i),
						() => (defaults[row.key] = 0)
					);
				}
			});
		}
	});

	/** `undefined` while Yomitan or the default entry is still loading. */
	function entryOf(row: Row): DefinitionEntry | null | undefined {
		const list = entries[scanOf(row)];
		const index = row.entryIndex ?? defaults[row.key];
		if (list === undefined || index === undefined || index < 0) return undefined;
		return list.find((e) => e.index === index) ?? null;
	}

	function pickEntry(row: Row, entry: DefinitionEntry) {
		if (row.term) setQueuedEntry(row.key, entry.index, scanOf(row));
		else setAdhocEntry(row.key, entry.index);
	}

	function setFormat(row: Row, name: string) {
		if (row.term) setQueuedFormat(row.key, name);
		else setAdhocFormat(row.key, name);
	}

	// Shift+hover on a term opens the definitions, as in the table.
	let picker = $state<{ row: Row; anchor: DOMRect } | null>(null);
	let hovered: { row: Row; el: HTMLElement } | null = null;

	function trackShift(e: KeyboardEvent) {
		if (e.key === 'Shift' && !e.repeat && hovered?.el.isConnected)
			picker = { row: hovered.row, anchor: hovered.el.getBoundingClientRect() };
	}

	function hover(e: MouseEvent, row: Row) {
		const el = e.currentTarget as HTMLElement;
		hovered = { row, el };
		if (e.shiftKey) picker = { row, anchor: el.getBoundingClientRect() };
	}

	async function moveFocus(e: KeyboardEvent) {
		if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
		e.preventDefault();
		const at = rows.findIndex((r) => r.key === focused?.key);
		const to = rows[Math.max(0, Math.min(rows.length - 1, at + (e.key === 'ArrowDown' ? 1 : -1)))];
		if (!to) return;
		focusedKey = to.key;
		await settled();
		const el = document.querySelector<HTMLElement>(`[data-row="${CSS.escape(to.key)}"]`);
		el?.focus();
		el?.scrollIntoView({ block: 'nearest' });
	}

	// Local mode shows frames; the cutter keeps the clips for mining.
	const showFrames = $derived($miningMode === 'local' && $localVideo !== null);
	let frames = $state<Record<string, string | null>>({});
	const requestedFrames = new Set<string>();
	const cueKey = (t: TimeStampDto) => `${t.start_secs}:${t.end_secs}`;
	const frameOf = (row: Row) =>
		row.sentence?.timestamp ? frames[cueKey(row.sentence.timestamp)] : null;

	$effect(() => {
		if (!showFrames) return;
		const cues = rows.flatMap((r) => (r.sentence?.timestamp ? [r.sentence.timestamp] : []));
		const fresh = cues.filter((c) => !requestedFrames.has(cueKey(c)));
		if (fresh.length === 0) return;
		for (const c of fresh) requestedFrames.add(cueKey(c));
		void prepareMedia(fresh)
			.catch(() => {})
			.then(async () => {
				// One at a time, so they don't all start ffmpeg at once.
				for (const c of fresh) frames[cueKey(c)] = await getLineMedia(c, 'frame').catch(() => null);
			});
	});

	let pane = $state<HTMLElement | null>(null);
	let enlarged = $state(false);
	let scoreOpen = $state(false);
	let audio: HTMLAudioElement | null = null;
	/** The line whose audio is playing, by `cueKey`. */
	let playing = $state<string | null>(null);

	function stopAudio() {
		audio?.pause();
		audio = null;
		playing = null;
	}

	$effect(() => stopAudio);

	// Each newly focused term opens at the top.
	$effect(() => {
		void focused?.key;
		untrack(() => {
			if (pane) pane.scrollTop = 0;
			enlarged = false;
			scoreOpen = false;
			stopAudio();
		});
	});

	/** Local mode plays the cut audio; otherwise the player seeks to the line. */
	async function play(row: Row) {
		const cue = row.sentence?.timestamp;
		if (!cue) return;
		if (!showFrames) {
			void seekTimestamp(cue.start_secs, cue.start_label);
			return;
		}
		const key = cueKey(cue);
		if (playing === key) {
			stopAudio();
			return;
		}
		const src = await getLineMedia(cue, 'audio').catch(() => null);
		if (!src) return;
		stopAudio();
		const clip = new Audio(src);
		clip.onended = () => {
			if (audio === clip) playing = null;
		};
		audio = clip;
		playing = key;
		void clip.play();
	}

	function frequencyNote(rank: number, points: number): string {
		const top = `the top ${horizon.toLocaleString()}`;
		return rank <= horizon
			? `#${rank.toLocaleString()} in the frequency lists, within ${top} your cards already cover, so it gets the full amount.`
			: `#${rank.toLocaleString()} in the frequency lists, past ${top} your cards cover. Rarer words get less, here ${Math.round(points)} of ${Math.round(frequencyPoints(1, horizon))}.`;
	}

	const title = $derived(
		review ? `Review picks · ${review.title}` : `Review selection · ${rows.length} terms`
	);
	const rule = $derived.by(() => {
		if (!review || !prefs) return 'In the order the lines play.';
		const limit =
			prefs.stop === 'count'
				? `up to ${prefs.limit} cards`
				: `every term scoring at least ${prefs.min_score}`;
		return `Auto mode picked ${limit}, best score first. Nothing is mined until you confirm.`;
	});
	const formatNames = $derived($cardFormats.map((f) => f.name));
</script>

{#snippet highlighted(row: Row)}{#each sentenceParts(row) as part, i (i)}{#if part.hit}<mark
			>{part.text}</mark
		>{:else}{part.text}{/if}{/each}{/snippet}

<svelte:window
	onkeydown={(e) => {
		trackShift(e);
		if (e.key === 'Escape') scoreOpen = false;
	}}
	onclick={() => (scoreOpen = false)}
/>

<Modal
	{title}
	width="min(68rem, 96%)"
	maxHeight="90%"
	onclose={() => (picker ? (picker = null) : onclose())}
>
	<p class="lede">{rule} Shift+hover a word for its definitions; ↑ and ↓ move through the list.</p>
	<div class="split">
		<div class="list">
			<!-- svelte-ignore a11y_no_noninteractive_element_interactions -- arrow keys move focus
			     between the row buttons inside it. -->
			<ul onkeydown={moveFocus}>
				{#each rows as row (row.key)}
					{@const entry = entryOf(row)}
					{@const frame = frameOf(row)}
					<li class="row" class:off={!row.ticked} class:focused={focused?.key === row.key}>
						<input
							type="checkbox"
							checked={row.ticked}
							aria-label={`Mine ${row.lemma}`}
							onchange={() => (row.ticked ? untick(row) : tick(row))}
						/>
						<button
							class="row-main"
							class:framed={showFrames}
							data-row={row.key}
							aria-current={focused?.key === row.key}
							onclick={() => (focusedKey = row.key)}
						>
							{#if showFrames}
								<span class="thumb">{#if frame}<img src={frame} alt="" />{/if}</span>
							{/if}
							<span class="text">
								<span class="line">
									<!-- svelte-ignore a11y_no_static_element_interactions -- Shift+hover
									     is a mouse shortcut; the detail pane lists the same entries. -->
									<span
										class="term"
										lang="ja"
										onmouseenter={(e) => hover(e, row)}
										onmouseleave={() => (hovered = null)}
										><Furigana surface={row.lemma} reading={row.reading} /></span
									>
									<span class="meaning">
										{entry
											? glossarySummary(entry.glossary_html, 3)
											: entry === null
												? 'No dictionary entry'
												: ''}
									</span>
								</span>
								<span class="line context">
									<span class="sentence" lang="ja"
										>{@render highlighted(row)}</span
									>
									{#if row.sentence?.timestamp}<span class="time"
											>{row.sentence.timestamp.start_label}</span
										>{/if}
								</span>
							</span>
							<span class="aside">
								{#if row.score}<span class="points" title="Auto mode score">{total(row.score)}</span
									>{/if}
								{#if row.formatName && row.formatName !== formatNames[0]}
									<span class="format-tag">{row.formatName}</span>
								{/if}
							</span>
						</button>
					</li>
				{/each}
			</ul>

			{#if review && next.length > 0}
				<section class="next">
					<h3>Next in line</h3>
					<p>Ranked just below the picks. Add one to mine it too.</p>
					<ul>
						{#each next as n (termKey(n.term))}
							{@const score = scoreOf(n.term)}
							<li>
								<span class="term" lang="ja"
									><Furigana surface={n.term.lemma_form} reading={n.term.lemma_reading} /></span
								>
								<span class="why">{MISSED[n.reason]}</span>
								{#if score}<span class="points">{total(score)}</span>{/if}
								<button class="add" onclick={() => addNext(n.term)}>Add</button>
							</li>
						{/each}
					</ul>
				</section>
			{/if}
		</div>

		<aside class="detail" aria-label="Selected term" bind:this={pane}>
			{#if focused}
				{@const row = focused}
				{@const list = entries[scanOf(row)]}
				{@const chosen = entryOf(row)}
				{@const frame = frameOf(row)}
				{@const cue = row.sentence?.timestamp ?? null}
				{@const changed =
					row.term !== null &&
					row.entryIndex !== undefined &&
					defaults[row.key] >= 0 &&
					row.entryIndex !== defaults[row.key]}
				<header class="detail-head" class:framed={showFrames}>
					{#if showFrames}
						<button
							class="frame-thumb"
							title={enlarged ? 'Shrink the frame' : 'Enlarge the frame'}
							aria-expanded={enlarged}
							onclick={() => (enlarged = !enlarged)}
						>
							{#if frame}<img src={frame} alt="Frame at this line" />{/if}
						</button>
					{/if}
					<div class="detail-title">
						<div class="detail-name">
							<!-- svelte-ignore a11y_no_static_element_interactions -- Shift+hover is a
							     mouse shortcut; Change entry opens the same chooser. -->
							<span
								class="detail-term"
								lang="ja"
								onmouseenter={(e) => hover(e, row)}
								onmouseleave={() => (hovered = null)}
								><Furigana surface={row.lemma} reading={row.reading} /></span
							>
							<span class="kind">
								{row.term
									? [posLabel(row.term.part_of_speech), row.term.jlpt_level]
											.filter(Boolean)
											.join(', ')
									: 'Not in the table'}
							</span>
						</div>
						<p class="detail-sentence" lang="ja">
							{@render highlighted(row)}
						</p>
						<div class="where">
							{#if cue && (showFrames || $playerConnected)}
								{@const isPlaying = playing === cueKey(cue)}
								<button
									class="play"
									title={isPlaying
										? 'Stop the sentence audio'
										: showFrames
											? 'Play the sentence audio'
											: 'Play this line in the player'}
									aria-pressed={isPlaying}
									onclick={() => void play(row)}
									><span class="play-icon" aria-hidden="true">{isPlaying ? '❚❚' : '▶'}</span>
									{cue.start_label}</button
								>
							{:else if cue}
								<span>{cue.start_label}</span>
							{/if}
							{#if row.occs.length > 1}
								<span class="stepper">
									<button
										disabled={!row.ticked || row.occIdx === 0}
										aria-label="Previous sentence"
										onclick={() =>
											pinOccurrence(row.key, { occIdx: row.occIdx - 1, userChosen: true })}
										>‹</button
									>
									Sentence {row.occIdx + 1} of {row.occs.length}
									<button
										disabled={!row.ticked || row.occIdx === row.occs.length - 1}
										aria-label="Next sentence"
										onclick={() =>
											pinOccurrence(row.key, { occIdx: row.occIdx + 1, userChosen: true })}
										>›</button
									>
								</span>
							{/if}
						</div>
					</div>
				</header>
				{#if showFrames && enlarged && frame}
					<button class="frame-large" title="Shrink the frame" onclick={() => (enlarged = false)}>
						<img src={frame} alt="Frame at this line" />
					</button>
				{/if}

				<div class="controls">
					{#if formatNames.length > 1}
						<div class="formats" role="radiogroup" aria-label="Card format">
							{#each formatNames as name (name)}
								<button
									role="radio"
									aria-checked={(row.formatName ?? formatNames[0]) === name}
									disabled={!row.ticked}
									onclick={() => setFormat(row, name)}>{name}</button
								>
							{/each}
						</div>
					{/if}
					{#if row.score && row.term}
						<div>
							<button
								class="score-btn"
								aria-expanded={scoreOpen}
								onclick={(e) => {
									e.stopPropagation();
									scoreOpen = !scoreOpen;
								}}
								>Why score {total(row.score)}<span class="caret" aria-hidden="true"
									>{scoreOpen ? '▴' : '▾'}</span
								></button
							>
							{#if scoreOpen}
								<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions --
								     the handler only fences the window's close-on-outside-click listener. -->
								<div class="score-pop" onclick={(e) => e.stopPropagation()}>
									<dl class="contributions">
										<div>
											<dt>{signed(row.score.frequency)}</dt>
											<dd>
												<strong>Frequency.</strong>
												{frequencyNote(harmonic(row.term), row.score.frequency)}
											</dd>
										</div>
										<div>
											<dt>{signed(row.score.wordType)}</dt>
											<dd>
												<strong>Word type.</strong> It's a {posLabel(
													row.term.part_of_speech
												).toLowerCase()}, which your settings give {signed(row.score.wordType)}.
											</dd>
										</div>
										<div>
											<dt>{signed(row.score.jlpt)}</dt>
											<dd>
												<strong>JLPT.</strong>
												{row.term.jlpt_level
													? `It's on the ${row.term.jlpt_level} list, which your settings give ${signed(row.score.jlpt)}.`
													: "It isn't on any JLPT list, so this adds nothing."}
											</dd>
										</div>
										<div class="sum">
											<dt>{total(row.score)}</dt>
											<dd><strong>Score</strong></dd>
										</div>
									</dl>
									<p class="quiet small">Word type and JLPT points are set in Mining → Auto Mode.</p>
								</div>
							{/if}
						</div>
					{/if}
				</div>

				<section class="definition">
					<div class="definition-head">
						{#if chosen}
							<span class="entry-name" lang="ja">{entryLabel(chosen, row.lemma)}</span>
							<span class="entry-state" class:changed>{changed ? 'Changed' : 'Default entry'}</span>
						{/if}
						{#if list && list.length > 1}
							<button
								class="link"
								disabled={!row.ticked}
								onclick={(e) => {
									e.stopPropagation();
									picker = { row, anchor: e.currentTarget.getBoundingClientRect() };
								}}>Change entry</button
							>
						{/if}
					</div>
					{#if list === undefined || chosen === undefined}
						<p class="quiet">Looking up the definition…</p>
					{:else if chosen === null}
						<p class="quiet">Yomitan has no entry for this word.</p>
					{:else}
						<div class="glossary"><Glossary html={chosen.glossary_html} /></div>
					{/if}
				</section>
			{:else}
				<p class="quiet">Nothing is selected.</p>
			{/if}
		</aside>
	</div>

	{#snippet footer()}
		<footer>
			<button
				class="primary"
				disabled={$queuedCount === 0 || !canMine}
				onclick={() => {
					onclose();
					onmine();
				}}
				>{$queuedCount === 0
					? 'Nothing to mine'
					: `Mine ${$queuedCount} card${$queuedCount === 1 ? '' : 's'}`}</button
			>
			<span class="count">{$queuedCount} of {rows.length} ticked</span>
			<span class="spacer"></span>
			{#if review}
				<button title="Mine nothing from this video. It isn't marked as processed." onclick={skipReview}
					>Skip video</button
				>
				<button class="quiet-btn" title="Close this; the picks stay selected" onclick={onclose}
					>Later</button
				>
			{:else}
				<button class="quiet-btn" onclick={onclose}>Close</button>
			{/if}
		</footer>
	{/snippet}
</Modal>

{#if picker}
	{@const row = picker.row}
	<DefinitionPopover
		text={scanOf(row)}
		label={row.lemma}
		anchor={picker.anchor}
		scale={$settings?.definition_scale ?? 1}
		pickedIndex={row.entryIndex ?? defaults[row.key]}
		onpick={(entry) => pickEntry(row, entry)}
		onclose={() => (picker = null)}
	/>
{/if}

<style>
	.lede {
		margin: 0;
		padding: 0 1rem;
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.split {
		display: grid;
		grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr);
		/* Fixed, so moving through the list doesn't resize the dialog. */
		height: 40rem;
		min-height: 0;
		border-top: 1px solid var(--border);
	}
	.list,
	.detail {
		min-height: 0;
		overflow-y: auto;
	}
	.list {
		border-right: 1px solid var(--border);
	}
	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.row {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		align-items: center;
		gap: 0.6rem;
		padding-left: 0.9rem;
		border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
	}
	.row:hover {
		background: var(--bg-hover);
	}
	.row.focused {
		background: color-mix(in srgb, var(--accent) 10%, transparent);
	}
	/* Outlines the whole row, tick box included, when its button has focus. */
	.row:has(.row-main:focus-visible) {
		outline: 2px solid var(--accent);
		outline-offset: -2px;
	}
	.row.off .row-main {
		opacity: 0.45;
	}
	.row-main {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
		gap: 0.75rem;
		min-width: 0;
		padding: 0.5rem 0.9rem 0.5rem 0;
		text-align: left;
		color: inherit;
		font: inherit;
		background: none;
		border: none;
		border-radius: 0;
		cursor: pointer;
	}
	.row-main.framed {
		grid-template-columns: 4.5rem minmax(0, 1fr) auto;
	}
	.row .row-main:hover:not(:disabled),
	.row .row-main:focus-visible {
		background: none;
		outline: none;
	}
	.thumb {
		display: block;
		aspect-ratio: 16 / 9;
		overflow: hidden;
		border-radius: var(--radius-sm);
		background: var(--bg-raised);
	}
	.thumb img,
	.frame-thumb img,
	.frame-large img {
		display: block;
		width: 100%;
		height: 100%;
		object-fit: cover;
	}
	.text {
		display: grid;
		gap: 0.1rem;
		min-width: 0;
	}
	.line {
		display: flex;
		align-items: baseline;
		gap: 0.6rem;
		min-width: 0;
	}
	.term {
		flex-shrink: 0;
		color: var(--term);
		font-size: 1.1rem;
	}
	.meaning,
	.sentence {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
	}
	.meaning {
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.context {
		font-size: 0.85rem;
	}
	.sentence {
		flex: 1;
	}
	mark {
		color: var(--term);
		background: none;
	}
	.time {
		flex-shrink: 0;
		font-size: 0.75rem;
		color: var(--text-muted);
		font-variant-numeric: tabular-nums;
	}
	.aside {
		display: grid;
		justify-items: end;
		gap: 0.2rem;
	}
	.points {
		font-size: 0.85rem;
		font-weight: 600;
		color: var(--text-muted);
		font-variant-numeric: tabular-nums;
	}
	.format-tag {
		padding: 0 0.35rem;
		font-size: 0.7rem;
		color: var(--accent);
		border: 1px solid color-mix(in srgb, var(--accent) 45%, transparent);
		border-radius: var(--radius-sm);
	}
	.next {
		padding: 1rem 0.9rem;
	}
	.next h3 {
		margin: 0;
		font-size: 0.9rem;
	}
	.next p {
		margin: 0.15rem 0 0.5rem;
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.next li {
		display: grid;
		grid-template-columns: minmax(4rem, auto) minmax(0, 1fr) auto auto;
		align-items: center;
		gap: 0.75rem;
		padding: 0.35rem 0;
		font-size: 0.85rem;
	}
	.next .term {
		font-size: 1rem;
	}
	.why {
		color: var(--text-muted);
	}
	.add {
		padding: 0.1rem 0.65rem;
		font-size: 0.8rem;
		color: var(--accent);
		background: none;
		border: 1px solid color-mix(in srgb, var(--accent) 50%, var(--border));
		border-radius: var(--radius);
		cursor: pointer;
	}
	.detail {
		display: flex;
		flex-direction: column;
		gap: 0.9rem;
		padding: 1rem 1.1rem 1.5rem;
	}
	.detail-head {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		grid-template-areas: 'name' 'sentence' 'where';
		gap: 0.5rem 0.9rem;
	}
	.detail-head.framed {
		grid-template-columns: 7.5rem minmax(0, 1fr);
		grid-template-areas: 'frame name' 'sentence sentence' 'where where';
	}
	.detail-title {
		display: contents;
	}
	.frame-thumb {
		grid-area: frame;
	}
	.detail-name {
		grid-area: name;
		align-self: end;
	}
	.detail-sentence {
		grid-area: sentence;
	}
	.detail-head .where {
		grid-area: where;
	}
	.frame-thumb,
	.frame-large {
		display: block;
		padding: 0;
		overflow: hidden;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		cursor: zoom-in;
	}
	.frame-thumb {
		aspect-ratio: 16 / 9;
		border-radius: var(--radius-sm);
	}
	.frame-thumb[aria-expanded='true'] {
		border-color: var(--accent);
	}
	.frame-large {
		flex-shrink: 0;
		aspect-ratio: 16 / 9;
		border-radius: var(--radius);
		cursor: zoom-out;
	}
	.detail-name {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		gap: 0.2rem 0.75rem;
	}
	.detail-term {
		color: var(--term);
		font-size: 1.6rem;
		line-height: 1.2;
	}
	.kind {
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.detail-sentence {
		margin: 0;
		font-size: 1rem;
		line-height: 1.6;
	}
	.where {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.75rem;
		font-size: 0.8rem;
		color: var(--text-muted);
		font-variant-numeric: tabular-nums;
	}
	.play-icon {
		display: inline-block;
		width: 1em;
		font-size: 0.75em;
		text-align: center;
	}
	.play {
		padding: 0.1rem 0.5rem;
		font-size: 0.8rem;
		color: var(--accent);
		background: none;
		border: 1px solid color-mix(in srgb, var(--accent) 45%, var(--border));
		border-radius: var(--radius);
		cursor: pointer;
		font-variant-numeric: tabular-nums;
	}
	.stepper {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
	}
	.stepper button {
		padding: 0 0.35rem;
		font-size: 0.85rem;
		line-height: 1.4;
		color: var(--text-muted);
		background: none;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		cursor: pointer;
	}
	.stepper button:hover:not(:disabled) {
		color: var(--text);
		border-color: var(--accent);
	}
	.stepper button:disabled {
		opacity: 0.35;
		cursor: default;
	}
	.controls {
		position: relative;
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.6rem;
	}
	.formats {
		display: inline-flex;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		overflow: hidden;
	}
	.formats button {
		padding: 0.2rem 0.7rem;
		font-size: 0.8rem;
		color: var(--text-muted);
		background: none;
		border: none;
		border-radius: 0;
		cursor: pointer;
	}
	.formats button + button {
		border-left: 1px solid var(--border);
	}
	.formats button[aria-checked='true'] {
		font-weight: 600;
		color: var(--bg-deep);
		background: var(--accent);
	}

	.score-btn {
		padding: 0.2rem 0.6rem;
		font-size: 0.8rem;
		color: var(--text-muted);
		background: none;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		cursor: pointer;
		font-variant-numeric: tabular-nums;
	}
	.score-btn:hover,
	.score-btn[aria-expanded='true'] {
		color: var(--text);
		border-color: var(--accent);
	}
	.caret {
		margin-left: 0.3rem;
		font-size: 0.65rem;
	}
	.score-pop {
		position: absolute;
		top: calc(100% + 0.4rem);
		left: 0;
		right: 0;
		z-index: 2;
		display: grid;
		gap: 0.5rem;
		padding: 0.75rem 0.85rem;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		box-shadow: var(--shadow-overlay);
	}
	.contributions {
		display: grid;
		gap: 0.45rem;
		margin: 0;
	}
	.contributions div {
		display: grid;
		grid-template-columns: 2.5rem minmax(0, 1fr);
		gap: 0.6rem;
		align-items: baseline;
	}
	.contributions dt {
		text-align: right;
		font-weight: 600;
		font-variant-numeric: tabular-nums;
	}
	.contributions dd {
		margin: 0;
		font-size: 0.85rem;
		line-height: 1.45;
		color: var(--text-muted);
	}
	.contributions strong {
		color: var(--text);
		font-weight: 600;
	}
	.contributions .sum {
		padding-top: 0.4rem;
		border-top: 1px solid var(--border);
	}
	.contributions .sum dt {
		color: var(--accent);
	}
	.definition {
		display: grid;
		gap: 0.5rem;
		padding-top: 0.9rem;
		border-top: 1px solid var(--border);
	}
	.definition-head {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 0.3rem 0.75rem;
	}
	.entry-name {
		font-size: 1rem;
		font-weight: 600;
	}
	.entry-state {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.entry-state.changed {
		color: var(--warning);
	}
	.link {
		padding: 0;
		font-size: 0.8rem;
		color: var(--accent);
		background: none;
		border: none;
		cursor: pointer;
	}
	.link:hover:not(:disabled) {
		text-decoration: underline;
	}
	.link:disabled {
		opacity: 0.5;
		cursor: default;
	}
	.glossary {
		font-size: 0.95rem;
		line-height: 1.55;
	}
	.quiet {
		margin: 0;
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.small {
		font-size: 0.8rem;
	}
	footer {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.75rem;
		padding: 0.75rem 1rem 0;
		border-top: 1px solid var(--border);
	}
	.count {
		font-size: 0.85rem;
		color: var(--text-muted);
		font-variant-numeric: tabular-nums;
	}
	.spacer {
		flex: 1;
	}
	footer .primary {
		font-weight: 600;
		color: var(--bg-deep);
		background: var(--accent);
		border-color: var(--accent);
	}
	.quiet-btn {
		background: none;
	}
	@media (max-width: 48rem) {
		.split {
			grid-template-columns: minmax(0, 1fr);
			grid-template-rows: minmax(0, 1fr) minmax(0, 1fr);
		}
		.list {
			border-right: none;
			border-bottom: 1px solid var(--border);
		}
	}
</style>
