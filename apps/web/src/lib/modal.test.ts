import { describe, it, expect, vi } from 'vitest';
import { modal } from './modal';

/**
 * The action only touches four things on the node, so a stub is enough and
 * keeps this in the existing node test project (no jsdom dependency for one
 * 40-line action).
 */
function fakeDialog(hasAutofocus = false) {
	const listeners: Record<string, ((e: unknown) => void)[]> = {};
	const node = {
		open: false,
		tabIndex: 0,
		focus: vi.fn(),
		querySelector: (sel: string) => (sel === '[autofocus]' && hasAutofocus ? {} : null),
		showModal: vi.fn(() => {
			node.open = true;
		}),
		close: vi.fn(() => {
			node.open = false;
		}),
		addEventListener: (type: string, fn: (e: unknown) => void) => {
			(listeners[type] ??= []).push(fn);
		},
		removeEventListener: (type: string, fn: (e: unknown) => void) => {
			listeners[type] = (listeners[type] ?? []).filter((f) => f !== fn);
		},
		emit(type: string, e: unknown) {
			for (const fn of listeners[type] ?? []) fn(e);
		},
		listenerCount: (type: string) => (listeners[type] ?? []).length
	};
	return node;
}

describe('modal action', () => {
	it('opens the dialog modally on mount', () => {
		const node = fakeDialog();
		modal(node as unknown as HTMLDialogElement, () => {});
		expect(node.showModal).toHaveBeenCalledOnce();
	});

	it('parks focus on the dialog rather than its first field', () => {
		// Otherwise showModal() drops a cursor into whatever input comes first —
		// which on the task card is the title, and focusing it suppresses the
		// effect that seeds the field, rendering a blank title.
		const node = fakeDialog();
		modal(node as unknown as HTMLDialogElement, () => {});
		expect(node.focus).toHaveBeenCalledOnce();
		expect(node.tabIndex).toBe(-1);
	});

	it('leaves focus alone when the content declares autofocus', () => {
		const node = fakeDialog(true);
		modal(node as unknown as HTMLDialogElement, () => {});
		expect(node.focus).not.toHaveBeenCalled();
	});

	it('closes on a backdrop click but not on a click inside the content', () => {
		const node = fakeDialog();
		const onClose = vi.fn();
		modal(node as unknown as HTMLDialogElement, onClose);

		node.emit('click', { target: {} }); // a descendant — content click
		expect(onClose).not.toHaveBeenCalled();

		node.emit('click', { target: node }); // the dialog itself — backdrop
		expect(onClose).toHaveBeenCalledOnce();
	});

	it('closes when the platform dismisses it (Escape fires `close`)', () => {
		const node = fakeDialog();
		const onClose = vi.fn();
		modal(node as unknown as HTMLDialogElement, onClose);

		node.emit('close', {});
		expect(onClose).toHaveBeenCalledOnce();
	});

	it('calls the latest callback after an update', () => {
		const node = fakeDialog();
		const first = vi.fn();
		const second = vi.fn();
		const action = modal(node as unknown as HTMLDialogElement, first);

		action.update(second);
		node.emit('close', {});
		expect(first).not.toHaveBeenCalled();
		expect(second).toHaveBeenCalledOnce();
	});

	it('does not re-enter the close handler when torn down', () => {
		// Teardown means the owner already unmounted us; firing onClose here would
		// push state the owner has already moved past (and can loop).
		const node = fakeDialog();
		const onClose = vi.fn();
		const action = modal(node as unknown as HTMLDialogElement, onClose);

		action.destroy();
		expect(node.close).toHaveBeenCalledOnce();
		expect(onClose).not.toHaveBeenCalled();
		expect(node.listenerCount('close')).toBe(0);
		expect(node.listenerCount('click')).toBe(0);
	});
});
