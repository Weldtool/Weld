<script lang="ts">
	import { Editor } from '@tiptap/core';
	import Link from '@tiptap/extension-link';
	import Placeholder from '@tiptap/extension-placeholder';
	import StarterKit from '@tiptap/starter-kit';
	import { Markdown } from 'tiptap-markdown';
	import { untrack } from 'svelte';

	interface Props {
		value: string;
		disabled?: boolean;
		fill?: boolean;
		onInput: (next: string) => void;
		onFocus?: () => void;
		onBlur?: () => void;
	}

	const { value, disabled = false, fill = false, onInput, onFocus, onBlur }: Props = $props();

	let element = $state<HTMLDivElement | null>(null);
	let editor: Editor | null = null;
	// Last markdown we emitted upward — used to break the parent ↔ editor cycle
	// (the parent passes `value` back after we emit, and we must not setContent on
	// our own echo).
	let lastEmitted = '';

	// `tiptap-markdown` augments `editor.storage.markdown` at runtime but does
	// not register a TS module declaration in the @tiptap/core types.
	function readMarkdown(ed: Editor): string {
		const storage = ed.storage as unknown as Record<string, { getMarkdown: () => string }>;
		return storage.markdown?.getMarkdown() ?? '';
	}

	$effect(() => {
		if (!element) return;
		// Create the editor exactly once per mount. The initial content/config are
		// read untracked so this effect depends only on `element` — otherwise typing
		// (which feeds `value` back through the parent) would re-run it and destroy
		// the editor mid-keystroke. External `value`/`disabled` changes are handled
		// by the reconciliation effects below.
		const ed = untrack(
			() =>
				new Editor({
					element: element!,
					extensions: [
						StarterKit.configure({ heading: { levels: [1, 2, 3] } }),
						Placeholder.configure({
							placeholder: disabled ? '' : 'Add a description… markdown shortcuts work.'
						}),
						Link.configure({ openOnClick: false, autolink: true }),
						Markdown.configure({ linkify: true, breaks: false, transformPastedText: true })
					],
					content: value,
					editable: !disabled,
					editorProps: {
						attributes: {
							class: fill
								? 'tiptap-content h-full overflow-y-auto px-3 py-2 focus:outline-none'
								: 'tiptap-content min-h-[120px] max-h-[220px] overflow-y-auto px-3 py-2 focus:outline-none'
						}
					},
					onUpdate({ editor }) {
						const md = readMarkdown(editor);
						if (md === lastEmitted) return;
						lastEmitted = md;
						onInput(md);
					},
					onFocus: () => onFocus?.(),
					onBlur: () => onBlur?.()
				})
		);
		editor = ed;
		lastEmitted = readMarkdown(ed);
		return () => {
			ed.destroy();
			if (editor === ed) editor = null;
		};
	});

	// Reconcile external updates (e.g. incoming WebSocket task.updated) without
	// disturbing the user mid-edit.
	$effect(() => {
		if (!editor) return;
		if (value === lastEmitted) return;
		if (editor.isFocused) return;
		lastEmitted = value;
		// `lastEmitted` is set above; the cycle-break guard in onUpdate will
		// swallow the resulting transaction without re-emitting upward.
		editor.commands.setContent(value);
	});

	$effect(() => {
		editor?.setEditable(!disabled);
	});
</script>

<div class={fill ? 'flex min-h-0 flex-1 flex-col gap-1' : 'space-y-1'}>
	<div class="flex items-center justify-between text-xs text-[var(--muted-foreground)]">
		<span>Description</span>
	</div>
	<div
		bind:this={element}
		class="{fill
			? 'flex min-h-0 flex-1 flex-col'
			: ''} rounded-md border border-[var(--border)] bg-[var(--card)] text-sm text-[var(--card-foreground)] focus-within:border-[var(--ring)] {disabled
			? 'opacity-60'
			: ''}"
	></div>
</div>

<style>
	:global(.tiptap-content p) {
		margin: 0.25rem 0;
	}
	:global(.tiptap-content p:first-child) {
		margin-top: 0;
	}
	:global(.tiptap-content h1) {
		font-size: 1.25rem;
		font-weight: 600;
		margin: 0.75rem 0 0.25rem;
	}
	:global(.tiptap-content h2) {
		font-size: 1.1rem;
		font-weight: 600;
		margin: 0.6rem 0 0.25rem;
	}
	:global(.tiptap-content h3) {
		font-size: 1rem;
		font-weight: 600;
		margin: 0.5rem 0 0.25rem;
	}
	:global(.tiptap-content ul),
	:global(.tiptap-content ol) {
		margin: 0.25rem 0;
		padding-left: 1.5rem;
	}
	:global(.tiptap-content ul) {
		list-style: disc;
	}
	:global(.tiptap-content ol) {
		list-style: decimal;
	}
	:global(.tiptap-content blockquote) {
		border-left: 3px solid var(--border);
		padding-left: 0.75rem;
		margin: 0.5rem 0;
		color: var(--muted-foreground);
	}
	:global(.tiptap-content code) {
		background: var(--muted);
		padding: 0.1rem 0.3rem;
		border-radius: 0.25rem;
		font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
		font-size: 0.85em;
	}
	:global(.tiptap-content pre) {
		background: var(--muted);
		padding: 0.6rem 0.8rem;
		border-radius: 0.375rem;
		margin: 0.5rem 0;
		overflow-x: auto;
	}
	:global(.tiptap-content pre code) {
		background: transparent;
		padding: 0;
	}
	:global(.tiptap-content a) {
		color: var(--primary);
		text-decoration: underline;
	}
	:global(.tiptap-content hr) {
		border: 0;
		border-top: 1px solid var(--border);
		margin: 0.75rem 0;
	}
	:global(.tiptap-content p.is-editor-empty:first-child::before) {
		content: attr(data-placeholder);
		color: var(--muted-foreground);
		float: left;
		pointer-events: none;
		height: 0;
	}
</style>
