<script lang="ts">
	// Log-scaled: frequency ranks span ~1..500k, so linear thumbs would cram the
	// useful low end into a few pixels.
	let {
		lo,
		hi,
		min,
		max,
		onchange
	}: {
		/** Data bounds (slider extent). `lo` is ≥1 (freqBounds floors it), so the
		 * log mapping is always defined. */
		lo: number;
		hi: number;
		/** Current selection. */
		min: number;
		max: number;
		onchange: (min: number, max: number) => void;
	} = $props();

	const clamp = (v: number, a: number, b: number) => Math.min(Math.max(v, a), b);

	// Thumbs are inset by half their width so the knobs sit fully inside the
	// track at either extreme instead of overhanging its edges.
	const PAD = 7;

	// value ↔ [0,1] fraction, log-scaled`).
	const toFrac = (v: number) => (Math.log(v) - Math.log(lo)) / (Math.log(hi) - Math.log(lo));
	const fromFrac = (f: number) => Math.round(Math.exp(Math.log(lo) + f * (Math.log(hi) - Math.log(lo))));

	const minFrac = $derived(clamp(toFrac(min), 0, 1));
	const maxFrac = $derived(clamp(toFrac(max), 0, 1));
	const thumbLeft = (f: number) => `calc(${PAD}px + ${f} * (100% - ${PAD * 2}px))`;

	let track: HTMLDivElement;
	let dragging: 'min' | 'max' | null = $state(null);

	function fracAt(e: PointerEvent): number {
		const rect = track.getBoundingClientRect();
		return clamp((e.clientX - rect.left - PAD) / (rect.width - PAD * 2), 0, 1);
	}

	function apply(which: 'min' | 'max', frac: number) {
		const v = clamp(fromFrac(frac), lo, hi);
		if (which === 'min') onchange(Math.min(v, max), max);
		else onchange(min, Math.max(v, min));
	}

	// One handler on the track: grab the nearest thumb and drag it. Pointer
	// capture keeps the drag alive outside the track. preventDefault stops the
	// WebView from starting a native text-selection drag on fast pulls, which
	// would show a 🚫 cursor and pointercancel our drag.
	function down(e: PointerEvent) {
		e.preventDefault();
		const f = fracAt(e);
		dragging = Math.abs(f - minFrac) <= Math.abs(f - maxFrac) ? 'min' : 'max';
		track.setPointerCapture(e.pointerId);
		apply(dragging, f);
	}
	function move(e: PointerEvent) {
		if (dragging) apply(dragging, fracAt(e));
	}
	function up() {
		dragging = null;
	}

	function key(which: 'min' | 'max', e: KeyboardEvent) {
		const step = e.key === 'ArrowLeft' ? -0.01 : e.key === 'ArrowRight' ? 0.01 : 0;
		if (step === 0) return;
		e.preventDefault();
		apply(which, clamp((which === 'min' ? minFrac : maxFrac) + step, 0, 1));
	}
</script>

<div
	class="track"
	bind:this={track}
	role="group"
	aria-label="Frequency range"
	onpointerdown={down}
	onpointermove={move}
	onpointerup={up}
	onpointercancel={up}
	onlostpointercapture={up}
>
	<div class="rail"></div>
	<div
		class="fill"
		style="left: {thumbLeft(minFrac)}; width: calc({maxFrac - minFrac} * (100% - {PAD * 2}px))"
	></div>
	<div
		class="thumb"
		role="slider"
		tabindex="0"
		aria-label="Minimum"
		aria-valuemin={lo}
		aria-valuemax={hi}
		aria-valuenow={min}
		style="left: {thumbLeft(minFrac)}"
		onkeydown={(e) => key('min', e)}
	></div>
	<div
		class="thumb"
		role="slider"
		tabindex="0"
		aria-label="Maximum"
		aria-valuemin={lo}
		aria-valuemax={hi}
		aria-valuenow={max}
		style="left: {thumbLeft(maxFrac)}"
		onkeydown={(e) => key('max', e)}
	></div>
</div>

<style>
	.track {
		position: relative;
		width: 14rem;
		height: 18px;
		margin: 0 0.35rem;
		cursor: pointer;
		touch-action: none;
		-webkit-user-select: none;
		user-select: none;
	}
	.rail {
		position: absolute;
		top: 50%;
		left: 7px;
		right: 7px;
		height: 4px;
		transform: translateY(-50%);
		background: var(--bg-hover);
		border-radius: 2px;
	}
	.fill {
		position: absolute;
		top: 50%;
		height: 4px;
		transform: translateY(-50%);
		background: var(--accent);
		border-radius: 2px;
	}
	.thumb {
		position: absolute;
		top: 50%;
		width: 12px;
		height: 12px;
		transform: translate(-50%, -50%);
		background: var(--text);
		border: 2px solid var(--accent);
		border-radius: 50%;
	}
	.thumb:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: 1px;
	}
</style>
