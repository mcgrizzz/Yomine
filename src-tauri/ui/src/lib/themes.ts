// Theme system: every theme supplies a value for each semantic token below,
// applied as `--<token>` inline on :root (overriding app.css's dracula
// fallback). Adding a built-in theme = dropping a JSON file in ./themes/.

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
	'term',
	'link',
	'info',
	'success',
	'warning',
	'danger',
	'status-ok',
	'status-warn',
	'status-error',
	'status-busy',
	'status-off',
	'know-unknown',
	'know-new',
	'know-young',
	'know-mature'
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
	{ label: 'Accents', tokens: ['accent', 'term', 'link', 'info', 'success', 'warning', 'danger'] },
	{
		label: 'Status dots',
		tokens: ['status-ok', 'status-warn', 'status-error', 'status-busy', 'status-off']
	},
	{ label: 'Anki states', tokens: ['know-unknown', 'know-new', 'know-young', 'know-mature'] }
];

interface ThemeFile {
	id: string;
	label: string;
	dark: boolean;
	colors: Record<string, string>;
}

// app.css's :root fallback block must stay in sync with themes/dracula.json.
const loaded = Object.values(
	import.meta.glob('./themes/*.json', { eager: true }) as Record<string, { default: ThemeFile }>
).map((m) => m.default);

const defaultColors = (dark: boolean): ThemeColors =>
	loaded.find((t) => t.id === (dark ? 'dracula' : 'paper'))!.colors as ThemeColors;

export function mergeColors(dark: boolean, colors: Record<string, string>): ThemeColors {
	const merged = { ...defaultColors(dark) };
	for (const token of TOKENS) if (colors[token] !== undefined) merged[token] = colors[token];
	return merged;
}

const completeTheme = (t: ThemeFile): Theme => ({
	id: t.id,
	label: t.label,
	dark: t.dark,
	colors: mergeColors(t.dark, t.colors)
});

// Dark themes first, then light; the defaults lead their group.
const builtinGroup = (dark: boolean, pinned: string): Theme[] =>
	loaded
		.filter((t) => t.dark === dark)
		.sort((a, b) =>
			a.id === pinned ? -1 : b.id === pinned ? 1 : a.label.localeCompare(b.label)
		)
		.map(completeTheme);

export const BUILTIN_THEMES: Theme[] = [
	...builtinGroup(true, 'dracula'),
	...builtinGroup(false, 'paper')
];

export const userThemeId = (name: string) => `user:${name}`;

export function themeFromUser(u: UserTheme): Theme {
	return completeTheme({ id: userThemeId(u.name), label: u.name, dark: u.dark, colors: u.colors });
}

export function allThemes(s: SettingsData | null): Theme[] {
	return [...BUILTIN_THEMES, ...(s?.user_themes ?? []).map(themeFromUser)];
}

export function resolveTheme(s: SettingsData | null): Theme {
	const dark = s?.dark_mode ?? true;
	const id = dark ? s?.theme_dark || 'dracula' : s?.theme_light || 'paper';
	return (
		allThemes(s).find((t) => t.id === id) ??
		BUILTIN_THEMES.find((t) => t.id === (dark ? 'dracula' : 'paper'))!
	);
}

export function applyTheme(theme: Theme): void {
	const style = document.documentElement.style;
	for (const token of TOKENS) style.setProperty(`--${token}`, theme.colors[token]);
	style.colorScheme = theme.dark ? 'dark' : 'light';
}
