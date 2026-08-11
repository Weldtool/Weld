/**
 * Turns a `<dialog>` into an open modal, using the platform instead of
 * reimplementing it. `showModal()` gives focus containment, `inert` background,
 * Escape-to-dismiss, and top-layer stacking (so a modal opened from inside
 * another sits above it, and Escape closes only the topmost) — all things a
 * hand-rolled `fixed inset-0` overlay has to fake and usually gets wrong.
 *
 * Usage:
 *
 * ```svelte
 * <dialog use:modal={close} class="… backdrop:bg-black/50">…</dialog>
 * ```
 *
 * The callback runs when the user dismisses the dialog — Escape, or a click on
 * the backdrop. It does not run when the dialog is removed by its own `{#if}`,
 * since that means the owner already knows it's closing.
 *
 * Pass no callback (`use:modal`) for a dialog that must not be dismissed — a
 * gated step the participant has to complete. Escape is then suppressed too,
 * which is the one place it's worth overriding the platform: the alternative is
 * a survey step that vanishes on a stray keypress.
 */
export function modal(node: HTMLDialogElement, onClose?: () => void) {
	let close = onClose;

	// A click on the backdrop targets the dialog element itself; a click on any
	// content inside targets a descendant. No stopPropagation needed.
	const onClick = (e: MouseEvent) => {
		if (close && e.target === node) close();
	};
	const onCloseEvent = () => close?.();
	// `cancel` fires before Escape closes the dialog; blocking it keeps a gated
	// dialog open.
	const onCancel = (e: Event) => {
		if (!close) e.preventDefault();
	};

	node.addEventListener('click', onClick);
	node.addEventListener('close', onCloseEvent);
	node.addEventListener('cancel', onCancel);
	node.showModal();

	// `showModal()` focuses the first focusable descendant, which for a detail
	// view is usually an editable field — opening a card should not drop a cursor
	// into its title. Park focus on the dialog itself instead (still inside the
	// focus trap, so Tab starts from the top). A modal that genuinely wants a
	// field focused says so with `autofocus`, and that keeps working.
	if (!node.querySelector('[autofocus]')) {
		node.tabIndex = -1;
		node.focus();
	}

	return {
		update(next?: () => void) {
			close = next;
		},
		destroy() {
			// Detach first: closing here is teardown, not a user dismissal, and must
			// not re-enter the owner's close handler.
			node.removeEventListener('click', onClick);
			node.removeEventListener('close', onCloseEvent);
			node.removeEventListener('cancel', onCancel);
			if (node.open) node.close();
		}
	};
}
