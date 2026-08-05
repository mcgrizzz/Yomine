<script lang="ts">
	import { untrack } from 'svelte';
	import { epubChapterModalOpen, epubPicker, loadAndStore } from '$lib/stores';
	import { filename } from '$lib/recents';
	import type { EpubChapter } from '$lib/ipc';

	let selected = $state(new Set<number>());
	let expandedChapters = $state(new Set<number>());

	// Open-time hydrate per the established modal pattern.
	$effect(() => {
		if ($epubChapterModalOpen)
			untrack(() => {
				selected = new Set();
				expandedChapters = new Set();
			});
	});

	const chapters = $derived($epubPicker?.book.chapters ?? []);
	const bookTitle = $derived(
		$epubPicker ? $epubPicker.book.title.trim() || filename($epubPicker.path) : ''
	);
	const allPartIds = $derived(chapters.flatMap((c) => c.parts.map((p) => p.id)));
	const selectedChars = $derived(
		chapters
			.flatMap((c) => c.parts)
			.filter((p) => selected.has(p.id))
			.reduce((sum, p) => sum + p.char_count, 0)
	);

	function selectedPartCount(chapter: EpubChapter): number {
		return chapter.parts.filter((p) => selected.has(p.id)).length;
	}

	function toggleChapter(chapter: EpubChapter) {
		const next = new Set(selected);
		const allSelected = selectedPartCount(chapter) === chapter.parts.length;
		for (const part of chapter.parts) {
			if (allSelected) next.delete(part.id);
			else next.add(part.id);
		}
		selected = next;
	}

	function togglePart(id: number) {
		const next = new Set(selected);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		selected = next;
	}

	function toggleExpanded(index: number) {
		const next = new Set(expandedChapters);
		if (next.has(index)) next.delete(index);
		else next.add(index);
		expandedChapters = next;
	}

	function close() {
		epubChapterModalOpen.set(false);
		epubPicker.set(null);
	}

	/** [1,2,3,5] → "1–3, 5" */
	function partRanges(numbers: number[]): string {
		const runs: string[] = [];
		let start = numbers[0];
		let prev = numbers[0];
		for (const n of numbers.slice(1)) {
			if (n === prev + 1) {
				prev = n;
				continue;
			}
			runs.push(start === prev ? `${start}` : `${start}–${prev}`);
			start = prev = n;
		}
		runs.push(start === prev ? `${start}` : `${start}–${prev}`);
		return runs.join(', ');
	}

	/** e.g. "All chapters", "Parts 1–3 of 7", "目次, 解説", "12 of 29 chapters". */
	function selectionLabel(): string {
		const chosen = chapters
			.map((chapter) => ({
				chapter,
				picked: chapter.parts
					.map((p, i) => (selected.has(p.id) ? i + 1 : 0))
					.filter((n) => n > 0)
			}))
			.filter(({ picked }) => picked.length > 0);

		const partsText = (c: EpubChapter, picked: number[]) =>
			`Parts ${partRanges(picked)} of ${c.parts.length}`;

		if (chosen.length === chapters.length && chosen.every(({ chapter, picked }) => picked.length === chapter.parts.length)) {
			return 'All chapters';
		}
		if (chosen.length === 1 && chosen[0].chapter.title.trim() === bookTitle) {
			const { chapter, picked } = chosen[0];
			if (picked.length < chapter.parts.length) return partsText(chapter, picked);
		}
		const listed = chosen
			.map(({ chapter, picked }) =>
				picked.length === chapter.parts.length
					? chapter.title
					: `${chapter.title} (${partsText(chapter, picked)})`
			)
			.join(', ');
		if (chosen.length > 3 || listed.length > 60) {
			return `${chosen.length} of ${chapters.length} chapters`;
		}
		return listed;
	}

	function mine() {
		const picker = $epubPicker;
		if (!picker || selected.size === 0) return;
		const indices = [...selected].sort((a, b) => a - b);
		const label = selectionLabel();
		close();
		void loadAndStore(picker.path, indices, label || null);
	}
</script>

<svelte:window onkeydown={(e) => $epubChapterModalOpen && e.key === 'Escape' && close()} />

{#if $epubChapterModalOpen && $epubPicker}
	<div
		class="backdrop"
		role="button"
		tabindex="-1"
		onclick={close}
		onkeydown={(e) => e.key === 'Escape' && close()}
	>
		<!-- Stop backdrop clicks inside the dialog from closing it. -->
		<div
			class="dialog"
			role="dialog"
			aria-modal="true"
			aria-label="Pick chapters to mine"
			tabindex="-1"
			onclick={(e) => e.stopPropagation()}
		>
			<header>
				<h2 title={bookTitle}>{bookTitle}</h2>
				<button class="close" aria-label="Close" onclick={close}>✕</button>
			</header>

			<div class="controls">
				<button class="mini" onclick={() => (selected = new Set(allPartIds))}>Select all</button>
				<button class="mini" disabled={selected.size === 0} onclick={() => (selected = new Set())}
					>Select none</button
				>
				<span class="summary">{selectedChars.toLocaleString()} chars selected</span>
			</div>

			<ul class="list">
				{#each chapters as chapter, chapterIndex (chapterIndex)}
					{@const picked = selectedPartCount(chapter)}
					{@const split = chapter.parts.length > 1}
					{@const seenCount = chapter.parts.filter((p) => p.seen).length}
					<li>
						<div class="row" class:selected={picked === chapter.parts.length}>
							<input
								type="checkbox"
								checked={picked === chapter.parts.length}
								indeterminate={picked > 0 && picked < chapter.parts.length}
								onchange={() => toggleChapter(chapter)}
							/>
							{#if split}
								<!-- Split chapters: title expands, the checkbox selects. -->
								<button class="title" onclick={() => toggleExpanded(chapterIndex)}>
									{chapter.title}
								</button>
								<button
									class="parts-hint"
									class:open={expandedChapters.has(chapterIndex)}
									onclick={() => toggleExpanded(chapterIndex)}
								>
									{picked > 0 ? `${picked}/${chapter.parts.length}` : chapter.parts.length} parts
									<span class="expand small" class:open={expandedChapters.has(chapterIndex)}
										>▶</span
									>
								</button>
							{:else}
								<button class="title" onclick={() => toggleChapter(chapter)}>{chapter.title}</button>
							{/if}
							{#if seenCount === chapter.parts.length}
								<span class="seen" title="Mined before">✓</span>
							{:else if seenCount > 0}
								<span class="seen partial" title="Partially mined ({seenCount}/{chapter.parts.length})">✓</span>
							{/if}
							<span class="chars">{chapter.char_count.toLocaleString()} chars</span>
						</div>
						{#if split && expandedChapters.has(chapterIndex)}
							<ul class="parts">
								{#each chapter.parts as part, partIndex (part.id)}
									<li>
										<label class="row part" class:selected={selected.has(part.id)}>
											<input
												type="checkbox"
												checked={selected.has(part.id)}
												onchange={() => togglePart(part.id)}
											/>
											<span class="part-title">Part {partIndex + 1}</span>
											{#if part.seen}<span class="seen" title="Mined before">✓</span>{/if}
											<span class="chars">{part.char_count.toLocaleString()} chars</span>
										</label>
									</li>
								{/each}
							</ul>
						{/if}
					</li>
				{/each}
			</ul>

			<footer>
				<button disabled={selected.size === 0} onclick={mine}>Mine</button>
				<button onclick={close}>Cancel</button>
			</footer>
		</div>
	</div>
{/if}

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--bg-deep) 70%, transparent);
		z-index: 50;
	}
	.dialog {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		width: min(560px, 92%);
		max-height: 82%;
		padding-bottom: 0.75rem;
		background: var(--bg-panel);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
	}
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.6rem;
		padding: 0.75rem 1rem 0;
	}
	h2 {
		margin: 0;
		font-size: 1rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.close {
		padding: 0.1rem 0.4rem;
	}
	.controls {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0 1rem;
	}
	.mini {
		padding: 0.15rem 0.5rem;
		font-size: 0.75rem;
	}
	.summary {
		margin-left: auto;
		font-size: 0.75rem;
		color: var(--text-muted);
	}
	.list {
		list-style: none;
		margin: 0;
		padding: 0 1rem;
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
		overflow-y: auto;
	}
	.row {
		display: flex;
		align-items: center;
		gap: 0.6rem;
		padding: 0.4rem 0.7rem;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: var(--radius);
	}
	.row:hover {
		background: var(--bg-hover);
		border-color: var(--accent);
	}
	.row.selected {
		border-color: var(--accent);
	}
	.expand {
		display: inline-block;
		color: var(--text-muted);
		font-size: 0.75rem;
		transition: transform 0.12s ease;
	}
	.expand.open {
		transform: rotate(90deg);
	}
	.expand.small {
		font-size: 0.6rem;
	}
	.title {
		flex: 1;
		display: flex;
		align-items: center;
		gap: 0.45rem;
		padding: 0;
		background: none;
		border: none;
		text-align: left;
		font-size: 0.85rem;
		color: var(--text);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		cursor: pointer;
	}
	.seen {
		font-size: 0.75rem;
		color: var(--know-mature, var(--info));
	}
	.seen.partial {
		opacity: 0.45;
	}
	.parts-hint {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
		padding: 0.15rem 0.55rem;
		font-size: 0.7rem;
		color: var(--text-muted);
		background: var(--bg-panel);
		border: 1px solid var(--border);
		border-radius: 999px;
		white-space: nowrap;
		cursor: pointer;
	}
	.parts-hint:hover,
	.parts-hint.open {
		border-color: var(--accent);
		color: var(--text);
	}
	.chars {
		font-size: 0.7rem;
		color: var(--text-muted);
		white-space: nowrap;
	}
	.parts {
		list-style: none;
		margin: 0.3rem 0 0;
		padding: 0 0 0 1.6rem;
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}
	.row.part {
		padding: 0.3rem 0.7rem;
		cursor: pointer;
	}
	.part-title {
		flex: 1;
		font-size: 0.8rem;
		color: var(--text);
	}
	footer {
		display: flex;
		gap: 0.5rem;
		padding: 0 1rem;
	}
</style>
