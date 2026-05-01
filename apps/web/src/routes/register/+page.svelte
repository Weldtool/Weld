<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { ApiError } from '$lib/api/client';
	import { auth } from '$lib/auth.svelte';

	let displayName = $state('');
	let email = $state('');
	let password = $state('');
	let submitting = $state(false);
	let error = $state<string | null>(null);

	function validate(): string | null {
		if (!displayName.trim()) return 'Please enter your name.';
		if (!email.trim()) return 'Email is required.';
		if (password.length < 12) return 'Password must be at least 12 characters.';
		return null;
	}

	async function onSubmit(event: SubmitEvent) {
		event.preventDefault();
		error = validate();
		if (error) return;

		submitting = true;
		try {
			await auth.register(email.trim(), password, displayName.trim());
			await goto(resolve('/'));
		} catch (err) {
			error =
				err instanceof ApiError
					? err.status === 409
						? 'That email is already registered.'
						: err.message
					: 'Something went wrong. Please try again.';
		} finally {
			submitting = false;
		}
	}
</script>

<div class="mx-auto flex w-full max-w-sm flex-col gap-6 py-12">
	<header class="space-y-1 text-center">
		<h1 class="text-2xl font-semibold tracking-tight">Create your account</h1>
		<p class="text-sm text-[var(--muted-foreground)]">Get started with Weld in seconds.</p>
	</header>

	<form class="flex flex-col gap-4" onsubmit={onSubmit} novalidate>
		<label class="flex flex-col gap-1.5 text-sm">
			<span class="font-medium">Name</span>
			<input
				type="text"
				autocomplete="name"
				required
				bind:value={displayName}
				class="rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
			/>
		</label>

		<label class="flex flex-col gap-1.5 text-sm">
			<span class="font-medium">Email</span>
			<input
				type="email"
				autocomplete="email"
				required
				bind:value={email}
				class="rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
			/>
		</label>

		<label class="flex flex-col gap-1.5 text-sm">
			<span class="font-medium">Password</span>
			<input
				type="password"
				autocomplete="new-password"
				required
				minlength={12}
				bind:value={password}
				class="rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
			/>
			<span class="text-xs text-[var(--muted-foreground)]">At least 12 characters.</span>
		</label>

		{#if error}
			<p
				role="alert"
				class="rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-600 dark:text-red-400"
			>
				{error}
			</p>
		{/if}

		<button
			type="submit"
			disabled={submitting}
			class="mt-2 inline-flex items-center justify-center rounded-md bg-[var(--primary)] px-4 py-2 text-sm font-medium text-[var(--primary-foreground)] transition-opacity hover:opacity-90 disabled:opacity-60"
		>
			{submitting ? 'Creating account…' : 'Create account'}
		</button>
	</form>

	<p class="text-center text-sm text-[var(--muted-foreground)]">
		Already have an account?
		<a href={resolve('/login')} class="font-medium text-[var(--foreground)] hover:underline"
			>Sign in</a
		>
	</p>
</div>
