// Split a segment's surface + hiragana reading into parts so that ruby (<rt>)
// furigana sits *only over kanji*, never over kana (okurigana). Japanese
// readings preserve okurigana verbatim, so the kana runs in the surface are
// exact anchors into the reading; the gaps between them are the kanji readings.
// e.g. 心ない / こころない → [心:こころ][ない:—], 取り扱い / とりあつかい →
// [取:と][り:—][扱:あつか][い:—]. Used by the term column and the sentence view.

// CJK ideographs (ext. A + unified + compatibility) + iteration/abbreviation marks.
const KANJI = /[㐀-鿿豈-﫿々〆]/;

/** A run of the surface: `rt` is the furigana for a kanji run, `null` for kana. */
export type FuriganaPart = { text: string; rt: string | null };

// Katakana → hiragana for anchor matching (the reading comes back as hiragana).
function toHiragana(s: string): string {
	let out = '';
	for (const c of s) {
		const code = c.codePointAt(0)!;
		out += code >= 0x30a1 && code <= 0x30f6 ? String.fromCodePoint(code - 0x60) : c;
	}
	return out;
}

export function furiganaParts(surface: string, reading: string): FuriganaPart[] {
	const plain: FuriganaPart[] = [{ text: surface, rt: null }];
	// Whole-over-whole fallback (only for unalignable irregular readings).
	const whole: FuriganaPart[] = [{ text: surface, rt: reading }];
	if (!reading || reading === surface || !KANJI.test(surface)) return plain;

	// Tokenize the surface into alternating kanji / kana runs (code-point aware).
	const runs: { text: string; kanji: boolean }[] = [];
	for (const c of surface) {
		const kanji = KANJI.test(c);
		const last = runs[runs.length - 1];
		if (last && last.kanji === kanji) last.text += c;
		else runs.push({ text: c, kanji });
	}

	const rNorm = toHiragana(reading);
	const parts: FuriganaPart[] = [];
	let r = 0; // cursor into reading / rNorm

	for (let i = 0; i < runs.length; i++) {
		const run = runs[i];
		if (!run.kanji) {
			// Kana run: must appear verbatim at the cursor — consume it, no furigana.
			const kana = toHiragana(run.text);
			if (!rNorm.startsWith(kana, r)) return whole;
			r += kana.length;
			parts.push({ text: run.text, rt: null });
		} else {
			// Kanji run: its reading extends to the next kana anchor (or the end).
			const next = runs[i + 1];
			const end = next ? rNorm.indexOf(toHiragana(next.text), r + 1) : reading.length;
			if (end <= r) return whole;
			parts.push({ text: run.text, rt: reading.slice(r, end) });
			r = end;
		}
	}
	return r === reading.length ? parts : whole;
}

export function furiganaText(surface: string, reading: string): string {
	return furiganaParts(surface, reading)
		.map((p) => (p.rt ? `${p.text}(${p.rt})` : p.text))
		.join('');
}
