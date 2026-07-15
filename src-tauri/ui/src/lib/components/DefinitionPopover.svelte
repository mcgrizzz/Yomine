<script module lang="ts">
	// Never write a literal style tag in this file (strings and comments count):
	// Svelte's preprocess regex would parse it as the component's style block.

	// Session-wide so reopening a term never refetches.
	const cache = new Map<string, DefinitionEntry[]>();

	const SIZE_KEY = 'yomine:definition-popover-size';
	const FORMAT_KEY = 'yomine:mine-format';

	function loadFormat(): string | null {
		try {
			return localStorage.getItem(FORMAT_KEY);
		} catch {
			return null;
		}
	}

	function loadSize(): { w: number; h: number } | null {
		try {
			return JSON.parse(localStorage.getItem(SIZE_KEY) ?? 'null');
		} catch {
			return null;
		}
	}

	/** Group the {frequencies} list (`(reading) Dict: value` per <li>) into one
	 * chip per dictionary, values joined — Yomitan-popup style. */
	function freqChips(html: string): { name: string; values: string }[] {
		const doc = new DOMParser().parseFromString(html, 'text/html');
		const groups = new Map<string, string[]>();
		for (const li of doc.querySelectorAll('li')) {
			const text = (li.textContent ?? '').trim();
			const idx = text.lastIndexOf(': ');
			if (idx < 0) continue;
			const name = text
				.slice(0, idx)
				.replace(/^\(.*\)\s*/, '')
				.trim();
			const value = text.slice(idx + 2).trim();
			const list = groups.get(name) ?? [];
			list.push(value);
			groups.set(name, list);
		}
		return [...groups.entries()].map(([name, values]) => ({ name, values: values.join(', ') }));
	}

	/** Anki-media refs can never resolve here (and every DOM insert re-requests
	 * them, spamming 404s), so anything that isn't a data: URI becomes `none`. */
	function scrubCssUrls(css: string): string {
		return css
			.replace(/@import[^;]*(;|$)/gi, '')
			.replace(/url\(\s*(?!['"]?data:)[^)]*\)/gi, 'none');
	}

	/** Defang third-party dictionary HTML. Embedded style tags are kept — Yomitan
	 * scopes them under `.yomitan-glossary` — but purged of external loads. */
	function sanitize(html: string): string {
		const doc = new DOMParser().parseFromString(html, 'text/html');
		doc.querySelectorAll('script, iframe, object, embed, link, meta').forEach((el) =>
			el.remove()
		);
		doc.querySelectorAll('style').forEach((el) => {
			el.textContent = scrubCssUrls(el.textContent ?? '');
		});
		for (const el of doc.body.querySelectorAll('*')) {
			for (const attr of [...el.attributes]) {
				const name = attr.name.toLowerCase();
				if (name.startsWith('on')) el.removeAttribute(attr.name);
				else if ((name === 'src' || name === 'href') && /^\s*javascript:/i.test(attr.value))
					el.removeAttribute(attr.name);
				else if (name === 'style' && /url\(/i.test(attr.value))
					el.setAttribute(attr.name, scrubCssUrls(attr.value));
			}
			if (el.tagName === 'A') el.removeAttribute('href');
			// An unresolvable image must take its Yomitan container along —
			// the styled wrapper alone renders as an empty white box.
			if (el.tagName === 'IMG' && !/^(https?:|data:)/i.test(el.getAttribute('src') ?? ''))
				(el.closest('a.gloss-image-link, span.gloss-image-container') ?? el).remove();
		}
		return doc.body.innerHTML;
	}
</script>

<script lang="ts">
	import { renderDefinition, type CardFormat, type DefinitionEntry } from '$lib/ipc';

	let {
		text,
		label = text,
		anchor,
		scale = 1,
		showMine,
		mineDisabled,
		mineTitle = 'Create an Anki card from the displayed sentence',
		formats = [],
		onmine,
		onqueue,
		onclose
	}: {
		/** What Yomitan scans (a lemma, or a sentence remainder to longest-match). */
		text: string;
		label?: string;
		anchor: DOMRect;
		scale?: number;
		showMine: boolean;
		mineDisabled: boolean;
		mineTitle?: string;
		/** Yomitan term card formats; >1 renders per-format buttons. */
		formats?: CardFormat[];
		onmine: (entryIndex: number, formatName?: string) => void;
		onqueue: (entryIndex: number, formatName?: string) => void;
		onclose: () => void;
	} = $props();

	const multiFormat = $derived(formats.length > 1);
	let chosenFormat = $state(loadFormat());
	const activeFormat = $derived(
		formats.find((f) => f.name === chosenFormat)?.name ?? formats[0]?.name
	);

	function setFormat(name: string) {
		chosenFormat = name;
		try {
			localStorage.setItem(FORMAT_KEY, name);
		} catch {
			// Best-effort: losing the remembered format is fine.
		}
	}

	let root = $state<HTMLElement | null>(null);
	let entries = $state<DefinitionEntry[] | null>(null);
	let error = $state<string | null>(null);
	let size = $state(loadSize());

	$effect(() => {
		const lookup = text;
		error = null;
		const hit = cache.get(lookup);
		if (hit) {
			entries = hit;
			return;
		}
		entries = null;
		renderDefinition(lookup).then(
			(result) => {
				cache.set(lookup, result);
				if (lookup === text) entries = result;
			},
			(e) => {
				if (lookup === text) error = String(e);
			}
		);
	});

	const pos = $derived.by(() => {
		// The Appearance UI scale is a root CSS zoom (+layout.svelte): rect and
		// window sizes are visual px, while our fixed top/left are zoomed px.
		const zoom = Number(getComputedStyle(document.documentElement).zoom) || 1;
		const vw = window.innerWidth / zoom;
		const vh = window.innerHeight / zoom;
		const a = { left: anchor.left / zoom, top: anchor.top / zoom, bottom: anchor.bottom / zoom };
		const width = Math.min(size?.w ?? 384, vw - 16);
		const height = size ? Math.min(size.h, vh - 16) : null;
		const left = Math.min(Math.max(a.left, 8), vw - width - 8);
		const spaceBelow = vh - a.bottom;
		if (spaceBelow < (height ?? 348) + 12 && a.top > spaceBelow) {
			return { left, width, height, anchored: `bottom: ${vh - a.top + 6}px;` };
		}
		return { left, width, height, anchored: `top: ${a.bottom + 6}px;` };
	});

	// The browser writes resize-handle drags as inline width/height — anything
	// there that differs from what we rendered is a user resize to remember.
	function saveResize() {
		if (!root) return;
		const w = Math.round(parseFloat(root.style.width));
		const h = Math.round(parseFloat(root.style.height));
		const appliedW = Math.round(pos.width);
		const appliedH = pos.height === null ? null : Math.round(pos.height);
		const changed =
			(Number.isFinite(w) && Math.abs(w - appliedW) > 1) ||
			(Number.isFinite(h) && (appliedH === null || Math.abs(h - appliedH) > 1));
		if (!changed) return;
		const next = {
			w: Number.isFinite(w) ? w : appliedW,
			h: Number.isFinite(h) ? h : root.offsetHeight
		};
		size = next;
		try {
			localStorage.setItem(SIZE_KEY, JSON.stringify(next));
		} catch {
			// Best-effort: losing the remembered size is fine.
		}
	}

	function scrolled(e: Event) {
		if (root && e.target instanceof Node && root.contains(e.target)) return;
		onclose();
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events --
     the click handler only fences the window's close-on-outside-click listener. -->
<div
	class="popover"
	class:sized={pos.height !== null}
	role="dialog"
	tabindex="-1"
	aria-label={`Definition of ${label}`}
	bind:this={root}
	style={`left: ${pos.left}px; width: ${pos.width}px; --def-scale: ${scale}; ` +
		(pos.height !== null ? `height: ${pos.height}px; ` : '') +
		pos.anchored}
	onclick={(e) => e.stopPropagation()}
	oncontextmenu={(e) => e.stopPropagation()}
>
	{#if showMine && multiFormat}
		<div class="format-row">
			<label for="popover-format">Card format</label>
			<select
				id="popover-format"
				value={activeFormat}
				onchange={(e) => setFormat(e.currentTarget.value)}
			>
				{#each formats as f (f.name)}
					<option value={f.name}>{f.name} — {f.deck} · {f.model}</option>
				{/each}
			</select>
		</div>
	{/if}
	<div class="body">
		{#if error !== null}
			<p class="status">Yomitan lookup failed: {error}</p>
		{:else if entries === null}
			<p class="status">Looking up 「{label}」…</p>
		{:else if entries.length === 0}
			<p class="status">No dictionary entry for 「{label}」</p>
		{:else}
			{#each entries as entry}
				<div class="entry">
					<div class="head" lang="ja">
						{#if entry.furigana_html.trim()}
							<span class="expression">{@html sanitize(entry.furigana_html)}</span>
						{:else}
							<span class="expression">{entry.expression || label}</span>
							{#if entry.reading && entry.reading !== entry.expression}
								<span class="reading">【{entry.reading}】</span>
							{/if}
						{/if}
						{#if showMine}
							<span class="actions">
								<button
									class="mine-btn"
									disabled={mineDisabled}
									title={multiFormat ? `${mineTitle} — format: ${activeFormat}` : mineTitle}
									onclick={() => {
										onmine(entry.index, multiFormat ? activeFormat : undefined);
										onclose();
									}}>+ Mine</button
								>
								<button
									class="mine-btn"
									title={'Select for batch mining using this definition' +
										(multiFormat ? ` — format: ${activeFormat}` : '')}
									onclick={() => {
										onqueue(entry.index, multiFormat ? activeFormat : undefined);
										onclose();
									}}>Queue</button
								>
							</span>
						{/if}
					</div>
					{#if entry.frequencies_html.trim()}
						{@const chips = freqChips(entry.frequencies_html)}
						{#if chips.length > 0}
							<div class="freqs" lang="ja">
								{#each chips as chip}
									<span class="freq-chip">
										<span class="freq-name">{chip.name}</span>
										<span class="freq-vals">{chip.values}</span>
									</span>
								{/each}
							</div>
						{/if}
					{/if}
					<div class="glossary" lang="ja">
						<!-- eslint-disable-next-line svelte/no-at-html-tags -- sanitized above -->
						{@html sanitize(entry.glossary_html)}
					</div>
				</div>
			{/each}
		{/if}
	</div>
</div>

<svelte:window
	onclick={onclose}
	onscrollcapture={scrolled}
	onkeydown={(e) => e.key === 'Escape' && onclose()}
	onpointerup={saveResize}
/>

<style>
	.popover {
		position: fixed;
		z-index: 100;
		display: flex;
		flex-direction: column;
		background: var(--bg-dark);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
		resize: both;
		overflow: hidden;
		min-width: 14rem;
		min-height: 4rem;
		max-width: calc(100vw - 1rem);
		max-height: calc(100vh - 1rem);
	}
	/* Scale the content, not .popover itself: the position math and remembered
	 * size (script above) work in unscaled px. */
	.body,
	.format-row {
		zoom: var(--def-scale, 1);
	}
	.format-row {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		padding: 0.4rem 0.75rem;
		font-size: 0.8rem;
		color: var(--comment);
		border-bottom: 1px solid var(--border);
	}
	.format-row select {
		flex: 1;
		min-width: 0;
		font-size: 0.8rem;
	}
	.body {
		flex: 1 1 auto;
		max-height: 20rem;
		overflow-y: auto;
		padding: 0.6rem 0.75rem;
	}
	.popover.sized .body {
		max-height: none;
	}
	.status {
		margin: 0;
		color: var(--comment);
		font-size: 0.9rem;
	}
	.entry + .entry {
		margin-top: 0.6rem;
		padding-top: 0.6rem;
		border-top: 1px solid var(--border);
	}
	.head {
		display: flex;
		align-items: baseline;
		gap: 0.35rem;
		margin-bottom: 0.25rem;
	}
	.expression {
		font-size: 1.3rem;
		color: var(--red);
	}
	.expression :global(ruby) {
		line-height: 1.9;
	}
	.expression :global(rt) {
		font-size: 0.55em;
		color: var(--comment);
	}
	.reading {
		color: var(--comment);
	}
	.freqs {
		display: flex;
		flex-wrap: wrap;
		gap: 0.3rem;
		margin: 0.15rem 0 0.4rem;
	}
	.freq-chip {
		display: inline-flex;
		align-items: stretch;
		font-size: 0.75rem;
		border: 1px solid color-mix(in srgb, var(--green) 45%, transparent);
		border-radius: var(--radius);
		overflow: hidden;
		white-space: nowrap;
	}
	.freq-name {
		padding: 0.05rem 0.4rem;
		background: color-mix(in srgb, var(--green) 30%, transparent);
		font-weight: 600;
	}
	.freq-vals {
		padding: 0.05rem 0.4rem;
		background: var(--bg-light);
	}
	.glossary {
		font-size: 0.95rem;
	}
	.glossary :global(ul),
	.glossary :global(ol) {
		margin: 0.2em 0;
		padding-left: 1.4em;
	}
	.glossary :global(img) {
		max-width: 100%;
	}
	/* The <i>(tags, Dictionary)</i> annotation Yomitan prefixes each sense with. */
	.glossary :global(.yomitan-glossary > i),
	.glossary :global(.yomitan-glossary ol > li > i) {
		color: var(--comment);
		font-size: 0.85em;
	}
	/* Mirrors the compact-glossary rules in Yomitan's structured-content.css:
	 * gloss alternatives inline, |-separated. */
	.glossary :global(ul[data-sc-content='glossary']),
	.glossary :global(.yomitan-glossary > ul),
	.glossary :global(.yomitan-glossary > ol > li > ul) {
		display: inline;
		margin: 0;
		padding-left: 0;
		list-style: none;
	}
	.glossary :global(ul[data-sc-content='glossary'] > li),
	.glossary :global(.yomitan-glossary > ul > li),
	.glossary :global(.yomitan-glossary > ol > li > ul > li) {
		display: inline;
	}
	.glossary :global(ul[data-sc-content='glossary'] > li:not(:first-child))::before,
	.glossary :global(.yomitan-glossary > ul > li:not(:first-child))::before,
	.glossary :global(.yomitan-glossary > ol > li > ul > li:not(:first-child))::before {
		content: ' | ';
		white-space: pre-wrap;
		color: var(--comment);
	}
	.actions {
		display: inline-flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: 0.3rem;
		margin-left: auto;
	}
	.mine-btn {
		cursor: pointer;
		padding: 0.1rem 0.45rem;
		background: var(--bg-light);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		color: var(--cyan);
		font-size: 0.75rem;
		white-space: nowrap;
	}
	.mine-btn:hover:not(:disabled) {
		background: var(--bg-lighter);
		border-color: var(--cyan);
	}
	.mine-btn:disabled {
		opacity: 0.5;
		cursor: default;
	}
</style>
