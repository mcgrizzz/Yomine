// Theme system: every theme supplies a value for each semantic token below,
// applied as `--<token>` inline on :root (overriding app.css's dracula
// fallback). Adding a built-in theme = appending one object to BUILTIN_THEMES.

import type { SettingsData, UserTheme } from '$lib/ipc';

export const TOKENS = [
	'bg-deep',
	'bg-panel',
	'bg',
	'bg-raised',
	'bg-hover',
	'text',
	'text-muted',
	'selection',
	'border',
	'accent',
	'link',
	'info',
	'success',
	'warning',
	'danger',
	'know-unknown',
	'know-new',
	'know-young',
	'know-mature',
	'pos-verb',
	'pos-noun',
	'pos-adjective',
	'pos-adverb',
	'pos-particle',
	'pos-other'
] as const;

export type TokenName = (typeof TOKENS)[number];
export type ThemeColors = Record<TokenName, string>;

export interface Theme {
	id: string;
	label: string;
	dark: boolean;
	colors: ThemeColors;
}

export const TOKEN_GROUPS: { label: string; tokens: TokenName[] }[] = [
	{ label: 'Surfaces', tokens: ['bg-deep', 'bg-panel', 'bg', 'bg-raised', 'bg-hover'] },
	{ label: 'Text', tokens: ['text', 'text-muted', 'selection', 'border'] },
	{ label: 'Accents', tokens: ['accent', 'link', 'info', 'success', 'warning', 'danger'] },
	{ label: 'Anki states', tokens: ['know-unknown', 'know-new', 'know-young', 'know-mature'] },
	{
		label: 'Parts of speech',
		tokens: ['pos-verb', 'pos-noun', 'pos-adjective', 'pos-adverb', 'pos-particle', 'pos-other']
	}
];

// Must stay in sync with the :root fallback block in app.css.
const DRACULA: Theme = {
	id: 'dracula',
	label: 'Dracula',
	dark: true,
	colors: {
		'bg-deep': '#191a21',
		'bg-panel': '#212335',
		bg: '#282a36',
		'bg-raised': '#343642',
		'bg-hover': '#424550',
		text: '#f8f8f2',
		'text-muted': '#6272a4',
		selection: '#44475a',
		border: '#424550',
		accent: '#8be9fd',
		link: '#62a0ea',
		info: '#62a0ea',
		success: '#50fa7b',
		warning: '#f1fa8c',
		danger: '#ff5555',
		'know-unknown': '#ff5555',
		'know-new': '#62a0ea',
		'know-young': '#ffb86c',
		'know-mature': '#50fa7b',
		'pos-verb': '#62a0ea',
		'pos-noun': '#50fa7b',
		'pos-adjective': '#ffb86c',
		'pos-adverb': '#bd93f9',
		'pos-particle': '#6272a4',
		'pos-other': '#f8f8f2'
	}
};

// Soft warm paper, well below pure white
const PAPER: Theme = {
	id: 'paper',
	label: 'Paper',
	dark: false,
	colors: {
		'bg-deep': '#d9d7cc',
		'bg-panel': '#e7e5da',
		bg: '#efede2',
		'bg-raised': '#f5f3e8',
		'bg-hover': '#faf8ee',
		text: '#282a36',
		'text-muted': '#6e7f95',
		selection: '#c8c8dc',
		border: '#c6c4b6',
		accent: '#2896c8',
		link: '#4682b4',
		info: '#4682b4',
		success: '#3caa64',
		warning: '#b0a030',
		danger: '#c85050',
		'know-unknown': '#c85050',
		'know-new': '#4682b4',
		'know-young': '#d8863c',
		'know-mature': '#3caa64',
		'pos-verb': '#4682b4',
		'pos-noun': '#3caa64',
		'pos-adjective': '#d8863c',
		'pos-adverb': '#9678dc',
		'pos-particle': '#6e7f95',
		'pos-other': '#282a36'
	}
};

export const BUILTIN_THEMES: Theme[] = [DRACULA, PAPER];

export const userThemeId = (name: string) => `user:${name}`;

/** Tokens missing from a saved theme (added after it was created) fall back
 * to the matching built-in default. */
export function themeFromUser(u: UserTheme): Theme {
	const base = u.dark ? DRACULA : PAPER;
	return {
		id: userThemeId(u.name),
		label: u.name,
		dark: u.dark,
		colors: { ...base.colors, ...u.colors } as ThemeColors
	};
}

export function allThemes(s: SettingsData | null): Theme[] {
	return [...BUILTIN_THEMES, ...(s?.user_themes ?? []).map(themeFromUser)];
}

export function resolveTheme(s: SettingsData | null): Theme {
	const dark = s?.dark_mode ?? true;
	const id = dark ? s?.theme_dark || 'dracula' : s?.theme_light || 'paper';
	return allThemes(s).find((t) => t.id === id) ?? (dark ? DRACULA : PAPER);
}

export function applyTheme(theme: Theme): void {
	const style = document.documentElement.style;
	for (const token of TOKENS) style.setProperty(`--${token}`, theme.colors[token]);
	style.colorScheme = theme.dark ? 'dark' : 'light';
}
