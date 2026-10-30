<script lang="ts">
	import { settingsDraft } from '$lib/settingsDraft.svelte';
	import type { TermOptions } from '$lib/ipc';
	import Modal from './Modal.svelte';
	import SettingsFooter from './SettingsFooter.svelte';
	import SettingsSection from './SettingsSection.svelte';
	import { defaultSettings, miningModalOpen, setTermOptions, settings } from '$lib/stores';

	const form = settingsDraft<TermOptions>({
		open: miningModalOpen,
		initial: {
			suru_as_verb: false,
			keep_affixed_nouns: false,
			verb_phrases: true,
			other_phrases: true,
			potential_forms: false,
			causative_forms: false,
			passive_forms: false
		},
		load: () => ($settings ? { ...$settings.term_options } : undefined)
	});
	const draft = $derived(form.value);

	const DERIVED: {
		key: 'potential_forms' | 'causative_forms' | 'passive_forms';
		label: string;
		example: string;
	}[] = [
		{ key: 'potential_forms', label: 'Potential', example: '読めて → 読める' },
		{ key: 'causative_forms', label: 'Causative', example: '驚かせて → 驚かせる' },
		{ key: 'passive_forms', label: 'Passive', example: '思われて → 思われる' }
	];
	const derivedOn = $derived(DERIVED.filter((d) => draft[d.key]).length);

	function setAllDerived(on: boolean) {
		for (const d of DERIVED) draft[d.key] = on;
	}

	async function save() {
		if (await setTermOptions(draft)) miningModalOpen.set(false);
	}

	function restoreDefault() {
		if ($defaultSettings) form.value = { ...$defaultSettings.term_options };
	}
</script>

<Modal
	open={$miningModalOpen}
	title="Mining"
	width="min(520px, 92%)"
	page
	onclose={form.request}
	oninteract={form.disarm}
>
	<div class="body">
		<p class="hint">
			These choose which term Yomine offers from a sentence. Saving reprocesses the open file. Your
			batch history keeps the terms as they were when you mined them.
		</p>

		<SettingsSection title="Noun + する">
			<div class="choices" role="radiogroup" aria-label="Noun + する">
				<label class="check">
					<input type="radio" value={false} bind:group={draft.suru_as_verb} />
					The noun <span class="example">勉強した → 勉強</span>
				</label>
				<label class="check">
					<input type="radio" value={true} bind:group={draft.suru_as_verb} />
					The verb <span class="example">勉強した → 勉強する</span>
				</label>
			</div>
			<p class="hint">Either way, an Anki card for 勉強 covers 勉強する, and the reverse.</p>
		</SettingsSection>

		<SettingsSection title="Nouns with a prefix or suffix">
			<div class="choices" role="radiogroup" aria-label="Nouns with a prefix or suffix">
				<label class="check">
					<input type="radio" value={false} bind:group={draft.keep_affixed_nouns} />
					The noun <span class="example">騎士達 → 騎士, 全マップ → マップ</span>
				</label>
				<label class="check">
					<input type="radio" value={true} bind:group={draft.keep_affixed_nouns} />
					The whole word <span class="example">騎士達, 全マップ</span>
				</label>
			</div>
			<p class="hint">
				Words the dictionary lists stay whole either way (子供たち), and names always drop their
				honorific (白羽さん → 白羽).
			</p>
		</SettingsSection>

		<SettingsSection title="Expressions">
			<div class="choices">
				<label class="check">
					<input type="checkbox" bind:checked={draft.verb_phrases} />
					Verb and adjective phrases <span class="example">気が引ける, ほうがいい</span>
				</label>
				<label class="check">
					<input type="checkbox" bind:checked={draft.other_phrases} />
					Other phrases <span class="example">これから, 目の前</span>
				</label>
			</div>
			<p class="hint">
				Only expressions the dictionary lists are built. When one is off, its words stay separate
				terms (気, 引ける). Words the tokenizer splits are joined either way when the dictionary
				lists them (二日酔い, 何気に).
			</p>
		</SettingsSection>

		<SettingsSection title="Derived verbs">
			<div class="choices">
				<label class="check">
					<input
						type="checkbox"
						checked={derivedOn === DERIVED.length}
						indeterminate={derivedOn > 0 && derivedOn < DERIVED.length}
						onchange={(e) => setAllDerived(e.currentTarget.checked)}
					/>
					All three
				</label>
				{#each DERIVED as d (d.key)}
					<label class="check nested">
						<input type="checkbox" bind:checked={draft[d.key]} />
						{d.label} <span class="example">{d.example}</span>
					</label>
				{/each}
			</div>
			<p class="hint">
				Off, a derived verb is its base verb's term (読めて → 読む). On, it's a term of its own when
				the dictionary lists it, one step from the base verb, and your 読む card no longer covers
				it.
			</p>
		</SettingsSection>
	</div>

	{#snippet footer()}
		<SettingsFooter
			{form}
			onsave={save}
			oncancel={form.revert}
			restore={{
				label: 'Reset Mining Settings',
				title: "Go back to Yomine's own choice of terms.",
				run: restoreDefault
			}}
		/>
	{/snippet}
</Modal>

<style>
	.body {
		display: grid;
		gap: 1.25rem;
		padding: 0 1rem;
	}
	p {
		margin: 0;
	}
	.hint {
		font-size: 0.85rem;
		line-height: 1.45;
		color: var(--text-muted);
	}
	.choices {
		display: grid;
		gap: 0.3rem;
	}
	.check {
		justify-self: start;
		display: flex;
		align-items: center;
		gap: 0.4rem;
	}
	.nested {
		padding-left: 1.4rem;
	}
	.example {
		font-size: 0.85rem;
		color: var(--text-muted);
	}
</style>
