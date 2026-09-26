import { untrack } from 'svelte';
import { fromStore, type Writable } from 'svelte/store';
import { dirtyGuard } from './dirtyGuard.svelte';

/** JSON with object keys sorted, so key order never counts as a change. */
function canonical(value: unknown): string {
	return JSON.stringify(value, (_, v) =>
		v && typeof v === 'object' && !Array.isArray(v)
			? Object.fromEntries(Object.entries(v).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)))
			: v
	);
}

/**
 * A settings dialog's staged edits and the saved values they're compared with. With
 * `load`, it reloads each time `open` becomes true; `load` returning undefined keeps
 * the current values.
 */
export function settingsDraft<T>(options: {
	open: Writable<boolean>;
	initial: T;
	load?: () => T | undefined | Promise<T | undefined>;
	/** Closes the dialog; defaults to setting `open` false. */
	close?: () => void;
	/** The part of the value that counts as a change; defaults to all of it. */
	compared?: (value: T) => unknown;
}) {
	const { open, load, compared = (v: T) => v } = options;
	let value = $state($state.snapshot(options.initial) as T);
	let saved = $state($state.snapshot(options.initial) as T);
	const savedKey = $derived(canonical(compared(saved)));
	const dirty = $derived(canonical(compared(value)) !== savedKey);
	const guard = dirtyGuard(() => dirty, options.close ?? (() => open.set(false)));

	function reset(next: T) {
		value = $state.snapshot(next) as T;
		saved = $state.snapshot(next) as T;
		guard.disarm();
	}

	async function reload() {
		const next = await load?.();
		if (next !== undefined) reset(next);
	}

	if (load) {
		const isOpen = fromStore(open);
		// untrack: `load` reading a store would reload, discarding edits, whenever it changes.
		$effect(() => {
			if (isOpen.current) untrack(() => void reload());
		});
	}

	return {
		get value() {
			return value;
		},
		set value(next: T) {
			value = $state.snapshot(next) as T;
		},
		get saved() {
			return saved;
		},
		get dirty() {
			return dirty;
		},
		get armed() {
			return guard.armed;
		},
		disarm: guard.disarm,
		/** Asks to close; with unsaved edits, the first ask only warns. */
		request: guard.request,
		reset,
		reload,
		revert: () => (value = $state.snapshot(saved) as T),
		/** Records the current values as saved. */
		commit: () => (saved = $state.snapshot(value) as T)
	};
}

export type SettingsDraft<T> = ReturnType<typeof settingsDraft<T>>;
