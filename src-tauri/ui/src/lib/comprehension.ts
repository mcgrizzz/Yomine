export function comprehensionColor(pct: number): string {
	return pct < 50
		? `color-mix(in oklab, var(--comp-low), var(--comp-mid) ${pct * 2}%)`
		: `color-mix(in oklab, var(--comp-mid), var(--comp-high) ${(pct - 50) * 2}%)`;
}
