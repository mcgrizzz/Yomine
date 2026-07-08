// POS-key → CSS color token. Shared by the term table and the sentence view.

export function posColor(posKey: string): string {
	switch (posKey) {
		case 'Verb':
		case 'SuruVerb':
			return 'var(--pos-verb)';
		case 'Noun':
			return 'var(--pos-noun)';
		case 'Adjective':
		case 'AdjectivalNoun':
			return 'var(--pos-adjective)';
		case 'Adverb':
			return 'var(--pos-adverb)';
		case 'Postposition':
			return 'var(--pos-particle)';
		default:
			return 'var(--pos-other)';
	}
}
