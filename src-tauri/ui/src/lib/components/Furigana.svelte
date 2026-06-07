<script lang="ts">
	// Renders surface text with furigana over kanji runs only (okurigana stays
	// bare). The whole word goes in ONE `<ruby>` with alternating base/<rt> pairs
	// (empty <rt> over kana) so per-kanji readings align, and the word is wrapped
	// in an inline-block box so a reading wider than its kanji can't overhang /
	// merge into the next word. Color/weight inherit from the parent. Shared by
	// the term column and the sentence view (see `$lib/furigana`).
	import { furiganaParts } from '$lib/furigana';

	let { surface, reading }: { surface: string; reading: string } = $props();
	const parts = $derived(furiganaParts(surface, reading));
	const hasRuby = $derived(parts.some((p) => p.rt !== null));
</script>

<!-- Compact markup: no whitespace inside the ruby (it would add empty bases). -->
<span class="word"
	>{#if hasRuby}<ruby
			>{#each parts as p, i (i)}{p.text}<rt>{p.rt ?? ''}</rt>{/each}</ruby
		>{:else}{surface}{/if}</span
>

<style>
	/* Atomic box per word → its furigana centers over the word and never
	   overhangs into the neighbor. Lines still break between words. */
	.word {
		display: inline-block;
		vertical-align: baseline;
	}
	ruby {
		/* Distribute the reading across the width it covers (spec default) rather
		   than bunching it in the centre — matters for jukugo like 警戒→けいかい
		   and for any whole-word fallback reading over a wider base. */
		ruby-align: space-around;
	}
	rt {
		font-size: 0.5em;
		font-weight: 400;
		color: var(--comment);
	}
</style>
