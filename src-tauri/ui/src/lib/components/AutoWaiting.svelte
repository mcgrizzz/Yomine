<span class="waiting" role="status">
	<span class="pulse" aria-hidden="true"></span>
	Waiting for new subs<span class="dots" aria-hidden="true"
		><span>.</span><span>.</span><span>.</span></span
	>
</span>

<style>
	.waiting {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
		font-size: 0.8rem;
		color: var(--text-muted);
		white-space: nowrap;
	}
	.pulse {
		width: 0.5rem;
		height: 0.5rem;
		border-radius: 50%;
		background: var(--accent);
		animation: pulse 1.6s ease-in-out infinite;
	}
	.dots span {
		animation: dot 1.4s infinite;
	}
	.dots span:nth-child(2) {
		animation-delay: 0.2s;
	}
	.dots span:nth-child(3) {
		animation-delay: 0.4s;
	}
	@keyframes pulse {
		0%,
		100% {
			opacity: 0.35;
			transform: scale(0.8);
		}
		50% {
			opacity: 1;
			transform: scale(1.1);
		}
	}
	@keyframes dot {
		0%,
		60%,
		100% {
			opacity: 0.2;
		}
		30% {
			opacity: 1;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.pulse,
		.dots span {
			animation: none;
		}
	}
</style>
