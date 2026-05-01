<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { ApiError } from '$lib/api/client';
	import { auth } from '$lib/auth.svelte';

	let email = $state('');
	let password = $state('');
	let submitting = $state(false);
	let error = $state<string | null>(null);

	async function onSubmit(event: SubmitEvent) {
		event.preventDefault();
		error = null;

		if (!email.trim() || !password) {
			error = 'Email and password are required.';
			return;
		}

		submitting = true;
		try {
			await auth.login(email.trim(), password);
			await goto(resolve('/'));
		} catch (err) {
			error =
				err instanceof ApiError
					? err.status === 401
						? 'Invalid email or password.'
						: err.message
					: 'Something went wrong. Please try again.';
		} finally {
			submitting = false;
		}
	}
</script>

<div class="mx-auto flex w-full max-w-sm flex-col gap-6 py-12">
	<header class="space-y-1 text-center">
		<h1 class="text-2xl font-semibold tracking-tight">Sign in</h1>
		<p class="text-sm text-[var(--muted-foreground)]">Welcome back to Weld.</p>
	</header>

	<form class="flex flex-col gap-4" onsubmit={onSubmit} novalidate>
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
				autocomplete="current-password"
				required
				bind:value={password}
				class="rounded-md border border-[var(--input)] bg-[var(--background)] px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]"
			/>
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
			{submitting ? 'Signing in…' : 'Sign in'}
		</button>
	</form>

	<p class="text-center text-sm text-[var(--muted-foreground)]">
		Don't have an account?
		<a href={resolve('/register')} class="font-medium text-[var(--foreground)] hover:underline"
			>Create one</a
		>
	</p>
</div>
