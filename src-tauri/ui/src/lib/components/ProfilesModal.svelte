<script lang="ts">
	import { relaunch } from '@tauri-apps/plugin-process';
	import Modal from './Modal.svelte';
	import * as ipc from '$lib/ipc';
	import { lastError, profilesModalOpen } from '$lib/stores';

	const RESTART_NOTE = 'Yomine restarts to switch profiles; the loaded file and any queued mining are lost.';

	type RowEdit = { slug: string; mode: 'rename' | 'copy'; value: string };

	let profiles = $state<ipc.Profile[]>([]);
	let newName = $state('');
	let editing = $state<RowEdit | null>(null);
	let confirmDelete = $state<string | null>(null);
	let switchArmed = $state<string | null>(null);
	let busy = $state(false);

	$effect(() => {
		if ($profilesModalOpen) void hydrate();
	});

	async function hydrate() {
		newName = '';
		editing = null;
		confirmDelete = null;
		switchArmed = null;
		profiles = (await ipc.listProfiles()).profiles;
	}

	async function run(action: () => Promise<{ requires_relaunch: boolean } | void>) {
		busy = true;
		try {
			const result = await action();
			if (result && result.requires_relaunch) {
				await relaunch();
				return;
			}
			await hydrate();
		} catch (err) {
			lastError.set({ title: 'Profiles', message: String(err), detail: null });
			busy = false;
			return;
		}
		busy = false;
	}

	const create = () => run(() => ipc.createProfile(newName));

	function startEdit(p: ipc.Profile, mode: 'rename' | 'copy') {
		editing = { slug: p.slug, mode, value: mode === 'copy' ? `${p.display_name} copy` : p.display_name };
		confirmDelete = null;
	}

	function commitEdit() {
		const edit = editing;
		if (!edit || edit.value.trim() === '') return;
		editing = null;
		void run(() =>
			edit.mode === 'copy'
				? ipc.copyProfile(edit.slug, edit.value)
				: ipc.renameProfile(edit.slug, edit.value)
		);
	}
</script>

<Modal
	open={$profilesModalOpen}
	title="Profiles"
	width="min(520px, 92%)"
	onclose={() => profilesModalOpen.set(false)}
>
	<p class="intro">
		Each profile keeps its own settings, ignore list and mined-card history. Frequency
		dictionaries are shared by all of them.
	</p>

	<div class="list">
		{#each profiles as p (p.slug)}
			<div class="profile-row">
				{#if editing?.slug === p.slug}
					{@const copying = editing.mode === 'copy'}
					<input
						class="rename"
						bind:value={editing.value}
						onkeydown={(e) => e.key === 'Enter' && commitEdit()}
						aria-label={copying ? `Name for the copy of ${p.display_name}` : `New name for ${p.display_name}`}
					/>
					<button class:primary={copying} onclick={commitEdit} disabled={editing.value.trim() === ''}>
						{copying ? 'Copy & Restart' : 'Save'}
					</button>
					<button aria-label="Cancel" onclick={() => (editing = null)}>✕</button>
				{:else}
					<span class="name">{p.display_name}</span>
					<span class="state">
						{#if p.active}
							<span class="chip badge">active</span>
						{:else if switchArmed === p.slug}
							<button
								class="chip primary"
								disabled={busy}
								onclick={() => run(() => ipc.switchProfile(p.slug))}>Restart now?</button
							>
							<button class="icon" aria-label="Cancel switch" onclick={() => (switchArmed = null)}
								>✕</button
							>
						{:else}
							<button class="chip" title={RESTART_NOTE} disabled={busy} onclick={() => (switchArmed = p.slug)}>
								Switch
							</button>
						{/if}
					</span>
					<span class="actions">
						<button
							class="icon"
							aria-label={`Copy ${p.display_name}`}
							title={`Duplicate ${p.display_name}'s settings, ignore list and mined history`}
							disabled={busy}
							onclick={() => startEdit(p, 'copy')}>⧉</button
						>
						<button
							class="icon"
							aria-label={`Rename ${p.display_name}`}
							disabled={busy}
							onclick={() => startEdit(p, 'rename')}>✎</button
						>
						{#if confirmDelete === p.slug}
							<button
								class="chip danger"
								disabled={busy}
								onclick={() => run(() => ipc.deleteProfile(p.slug))}>Delete everything?</button
							>
							<button class="icon" aria-label="Keep profile" onclick={() => (confirmDelete = null)}
								>✕</button
							>
						{:else}
							<button
								class="icon remove"
								aria-label={`Delete ${p.display_name}`}
								title={profiles.length < 2
									? 'The last profile cannot be deleted'
									: `Permanently deletes ${p.display_name}'s settings, ignore list and mined history`}
								disabled={busy || profiles.length < 2}
								onclick={() => (confirmDelete = p.slug)}>🗑</button
							>
						{/if}
					</span>
				{/if}
			</div>
		{/each}
	</div>

	{#snippet footer()}
		<div class="status">{RESTART_NOTE}</div>
		<footer>
			<input
				bind:value={newName}
				onkeydown={(e) => e.key === 'Enter' && newName.trim() !== '' && create()}
				placeholder="New profile name…"
				aria-label="New profile name"
			/>
			<button class="primary" disabled={busy || newName.trim() === ''} onclick={create}>
				Create &amp; Restart
			</button>
		</footer>
	{/snippet}
</Modal>

<style>
	.intro {
		margin: 0;
		padding: 0 1rem;
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.list {
		display: flex;
		flex-direction: column;
		padding: 0 1rem;
	}
	.profile-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		min-height: 2.5rem;
		border-bottom: 1px solid var(--border);
	}
	.state {
		display: inline-flex;
		align-items: center;
		justify-content: flex-end;
		gap: 0.3rem;
		min-width: 5rem;
	}
	.chip {
		font-size: 0.75rem;
		line-height: 1.5;
		padding: 0.15rem 0.5rem;
		border-radius: var(--radius-pill);
		white-space: nowrap;
	}
	.badge {
		background: color-mix(in srgb, var(--success) 15%, transparent);
		color: var(--success);
		border: 1px solid transparent;
	}
	button.chip {
		background: none;
		border: 1px solid color-mix(in srgb, currentColor 35%, transparent);
		color: var(--text);
	}
	button.chip:hover:not(:disabled) {
		background: color-mix(in srgb, currentColor 10%, transparent);
		border-color: currentColor;
	}
	/* Labels stay --text: accent and danger fail 4.5:1 on it in some of the 14 themes. */
	button.chip.primary {
		background: color-mix(in srgb, var(--accent) 10%, transparent);
		border-color: var(--accent);
	}
	button.chip.danger {
		background: color-mix(in srgb, var(--danger) 10%, transparent);
		border-color: var(--danger);
	}
	.name {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.rename {
		flex: 1;
		min-width: 0;
	}
	.actions {
		display: flex;
		align-items: center;
		gap: 0.3rem;
	}
	.icon {
		min-width: 24px;
		min-height: 24px;
		padding: 0 0.25rem;
		background: none;
		border: none;
		color: var(--text-muted);
	}
	.icon:hover:not(:disabled) {
		color: var(--text);
		background: none;
	}
	.icon.remove:hover:not(:disabled) {
		color: var(--danger);
	}
	.status {
		padding: 0 1rem;
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	footer {
		display: flex;
		gap: 0.5rem;
		align-items: center;
		padding: 0 1rem;
	}
	footer input {
		flex: 1;
		min-width: 0;
	}
</style>
