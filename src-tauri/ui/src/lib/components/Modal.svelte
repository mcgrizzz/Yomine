<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		width: string;
		onclose: () => void;
		open?: boolean;
		maxHeight?: string;
		/** Content owns its own bottom edge — for a last child with its own padding and border. */
		flush?: boolean;
		oninteract?: () => void;
		actions?: Snippet;
		children: Snippet;
	}

	let {
		title,
		width,
		onclose,
		open = true,
		maxHeight,
		flush = false,
		oninteract,
		actions,
		children
	}: Props = $props();

	const headingId = $props.id();
	const FOCUSABLE =
		'a[href], button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';

	let dialog = $state<HTMLDivElement | null>(null);

	$effect(() => {
		if (!open || !dialog) return;
		const invoker = document.activeElement;
		dialog.focus();
		return () => {
			// Modals open from menus that unmount, so the invoker is usually already gone.
			if (invoker instanceof HTMLElement && invoker.isConnected) invoker.focus();
		};
	});

	function onclick(e: MouseEvent) {
		if (e.target === e.currentTarget) onclose();
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

<!-- Backs up the backdrop's keydown for the frame before the focus effect runs. -->
<svelte:window onkeydown={(e) => open && e.key === 'Escape' && onclose()} />

{#if open}
	<div class="backdrop" role="button" tabindex="-1" {onclick} {onkeydown}>
		<div
			class="dialog"
			class:flush
			role="dialog"
			aria-modal="true"
			aria-labelledby={headingId}
			tabindex="-1"
			bind:this={dialog}
			style="width: {width};{maxHeight ? ` max-height: ${maxHeight};` : ''}"
		>
			<header>
				<h2 id={headingId} title={title}>{title}</h2>
				{#if actions}
					<div class="head-actions">{@render actions()}</div>
				{/if}
				<!-- Without stopPropagation the backdrop's oninteract disarms the guard this just armed. -->
				<button
					class="close"
					aria-label="Close"
					onclick={(e) => {
						e.stopPropagation();
						onclose();
					}}>✕</button
				>
			</header>
			{@render children()}
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
		padding-bottom: 0.75rem;
		overflow-y: auto;
		background: var(--bg-panel);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
	}
	.dialog.flush {
		gap: 0;
		padding-bottom: 0;
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
		margin: 0;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 1.05rem;
		color: var(--accent);
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
</style>
