// Red (0%) → yellow (50%) → green (100%), desaturated by blending gray(140) at
// gamma 0.6 over the base — premultiplied "over" is `ch * 0.4 + 84` per channel.
export function comprehensionColor(pct: number): string {
	const base =
		pct >= 50 ? [180 * (1 - (pct - 50) / 50), 180, 60] : [180, 180 * (pct / 50), 60];
	const [r, g, b] = base.map((c) => Math.round(c * 0.4 + 84));
	return `rgb(${r}, ${g}, ${b})`;
}
