<script lang="ts">
	import type { Snippet } from 'svelte';
	import { setSettingsClose } from '$lib/stores/modals';

	interface Props {
		title: string;
		width: string;
		/** Percent, not vh: vh ignores the root `zoom` the Appearance scale applies. */
		maxHeight?: string;
		/** A fixed height, so the dialog keeps its size as its content changes. */
		height?: string;
		/** Shows it as a page of the Settings window instead of a dialog of its own. */
		page?: boolean;
		/** Hides it while keeping its content, scroll position and edits. */
		hidden?: boolean;
		onclose: () => void;
		open?: boolean;
		/** Content owns its own bottom edge — for a footer with its own padding and border. */
		flush?: boolean;
		/** False leaves the backdrop inert, so a stray click cannot dismiss. */
		dismissible?: boolean;
		oninteract?: () => void;
		/** Shows a Back button before the title, for a dialog with a second view. */
		onback?: () => void;
		actions?: Snippet;
		footer?: Snippet;
		children: Snippet;
	}

	let {
		title,
		width,
		maxHeight = '85%',
		height,
		page = false,
		hidden = false,
		onclose,
		open = true,
		flush = false,
		dismissible = true,
		oninteract,
		onback,
		actions,
		footer,
		children
	}: Props = $props();

	const headingId = $props.id();
	const FOCUSABLE =
		'a[href], button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';

	let dialog = $state<HTMLDivElement | null>(null);

	$effect(() => {
		if (page && open) return setSettingsClose(onclose);
	});

	$effect(() => {
		if (!open || !dialog || hidden) return;
		const invoker = document.activeElement;
		dialog.focus();
		return () => {
			// Modals open from menus that unmount, so the invoker is usually already gone.
			if (invoker instanceof HTMLElement && invoker.isConnected) invoker.focus();
		};
	});

	// A click fires on the common ancestor of press and release, so a drag that
	// starts inside the dialog and ends on the backdrop would otherwise dismiss it.
	let pressedBackdrop = false;

	function onmousedown(e: MouseEvent) {
		pressedBackdrop = e.target === e.currentTarget;
	}

	function onclick(e: MouseEvent) {
		if (e.target === e.currentTarget && pressedBackdrop) onclose();
		else oninteract?.();
	}

	function onkeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			e.stopPropagation();
			onclose();
			return;
		}
		if (e.key !== 'Tab' || !dialog) return;
		const stops = [...dialog.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
			(el) => el.offsetParent !== null
		);
		if (stops.length === 0) return;
		const first = stops[0];
		const last = stops[stops.length - 1];
		const active = document.activeElement;
		if (e.shiftKey && (active === first || active === dialog)) {
			e.preventDefault();
			last.focus();
		} else if (!e.shiftKey && active === last) {
			e.preventDefault();
			first.focus();
		}
	}
</script>

<!-- Backs up the backdrop's keydown for the frame before the focus effect runs. A page leaves
     Escape to the window around it. -->
<svelte:window onkeydown={(e) => open && !page && !hidden && e.key === 'Escape' && onclose()} />

{#snippet heading(closable: boolean)}
	<header>
		{#if onback}
			<button class="back" onclick={onback}><span aria-hidden="true">←</span> Back</button>
		{/if}
		<h2 id={headingId} {title}>{title}</h2>
		{#if actions}
			<div class="head-actions">{@render actions()}</div>
		{/if}
		{#if closable}
			<!-- Without stopPropagation the backdrop's oninteract disarms the guard this just armed. -->
			<button
				class="close"
				aria-label="Close"
				onclick={(e) => {
					e.stopPropagation();
					onclose();
				}}>✕</button
			>
		{/if}
	</header>
{/snippet}

{#snippet dialogBox()}
	<div
		class="dialog"
		class:flush
		role="dialog"
		aria-modal="true"
		aria-labelledby={headingId}
		tabindex="-1"
		bind:this={dialog}
		style:width
		style:max-height={maxHeight}
		style:height
	>
		{@render heading(true)}
		<div class="content">{@render children()}</div>
		{@render footer?.()}
	</div>
{/snippet}

{#if open}
	{#if page}
		<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -- a
		     click only disarms the unsaved-changes warning, which keys don't arm. -->
		<section class="page" class:flush aria-labelledby={headingId} onclick={() => oninteract?.()}>
			{@render heading(false)}
			<div class="content">{@render children()}</div>
			{@render footer?.()}
		</section>
	{:else if dismissible}
		<div
			class="backdrop"
			class:hidden
			role="button"
			tabindex="-1"
			{onmousedown}
			{onclick}
			{onkeydown}
		>
			{@render dialogBox()}
		</div>
	{:else}
		<div class="backdrop" class:hidden>{@render dialogBox()}</div>
	{/if}
{/if}

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--bg-deep) 70%, transparent);
		z-index: var(--z-modal);
	}
	/* Not display: none, which would drop the content's scroll positions. */
	.backdrop.hidden {
		visibility: hidden;
	}
	.dialog {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		padding-bottom: 0.75rem;
		background: var(--bg-panel);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		box-shadow: var(--shadow-modal);
	}
	.page {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: 0.6rem;
		min-height: 0;
		padding-bottom: 0.75rem;
	}
	.dialog.flush,
	.page.flush {
		gap: 0;
		padding-bottom: 0;
	}
	.content {
		flex: 1 1 auto;
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		overflow-y: auto;
		/* Flexbox: without this the content refuses to shrink, pushing the footer
		   off-screen instead of scrolling. */
		min-height: 0;
	}
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.6rem;
		padding: 0.75rem 1rem;
		border-bottom: 1px solid var(--border);
	}
	header h2 {
		margin: 0 auto 0 0;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 1.05rem;
		color: var(--accent);
	}
	.page header h2 {
		font-size: 0.95rem;
		color: var(--text);
	}
	.head-actions {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		margin-left: auto;
	}
	.close {
		padding: 0.1rem 0.4rem;
	}
	.back {
		flex-shrink: 0;
		padding: 0.2rem 0.6rem;
	}
</style>
