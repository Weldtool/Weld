import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$app/environment', () => ({ browser: true }));

import { chatDrafts, draftKey } from './chat-drafts.svelte';

/** Minimal localStorage stand-in — the node test env has no DOM. */
function installStorage(): Map<string, string> {
	const store = new Map<string, string>();
	const stub = {
		getItem: (k: string) => store.get(k) ?? null,
		setItem: (k: string, v: string) => void store.set(k, v),
		removeItem: (k: string) => void store.delete(k),
		key: (i: number) => [...store.keys()][i] ?? null,
		clear: () => store.clear(),
		get length() {
			return store.size;
		}
	};
	// Object.keys(localStorage) is how clearAll enumerates; a plain object with
	// the entries mixed in reproduces that behaviour.
	vi.stubGlobal(
		'localStorage',
		new Proxy(stub, {
			ownKeys: () => [...store.keys()],
			getOwnPropertyDescriptor: () => ({ enumerable: true, configurable: true })
		})
	);
	return store;
}

describe('chatDrafts', () => {
	let storage: Map<string, string>;

	beforeEach(() => {
		storage = installStorage();
		chatDrafts.clearAll();
	});

	it('keeps a channel draft separate from its threads', () => {
		const channel = draftKey('chan-1');
		const thread = draftKey('chan-1', 'msg-9');
		expect(channel).not.toBe(thread);

		chatDrafts.set(channel, 'in the channel');
		chatDrafts.set(thread, 'in the thread');

		expect(chatDrafts.get(channel)).toBe('in the channel');
		expect(chatDrafts.get(thread)).toBe('in the thread');
	});

	it('survives the composer being destroyed and rebuilt', () => {
		// Navigating away and back is just a fresh read of the same key.
		chatDrafts.set(draftKey('chan-1'), 'half a sentence');
		expect(chatDrafts.get(draftKey('chan-1'))).toBe('half a sentence');
	});

	it('reads a draft back from storage when nothing is held in memory', () => {
		storage.set('weld:chat-draft:chan-1', 'from a previous page load');
		expect(chatDrafts.get('chan-1')).toBe('from a previous page load');
	});

	it('returns empty for a composer with no draft', () => {
		expect(chatDrafts.get('never-typed-in')).toBe('');
	});

	it('clears the stored draft when the box is emptied', () => {
		chatDrafts.set('chan-1', 'typed');
		chatDrafts.set('chan-1', '');
		expect(chatDrafts.get('chan-1')).toBe('');
		expect(storage.has('weld:chat-draft:chan-1')).toBe(false);
	});

	it('drops the draft once the message is sent', () => {
		chatDrafts.set('chan-1', 'about to send');
		chatDrafts.clear('chan-1');
		expect(chatDrafts.get('chan-1')).toBe('');
		expect(storage.has('weld:chat-draft:chan-1')).toBe(false);
	});

	it('clearAll removes drafts without touching other keys', () => {
		chatDrafts.set('chan-1', 'a');
		chatDrafts.set('chan-2', 'b');
		storage.set('weld:agent-collapsed', '1');

		chatDrafts.clearAll();

		expect(chatDrafts.get('chan-1')).toBe('');
		expect(chatDrafts.get('chan-2')).toBe('');
		expect(storage.get('weld:agent-collapsed')).toBe('1');
	});

	it('survives storage being unavailable', () => {
		vi.stubGlobal('localStorage', {
			getItem: () => null,
			setItem: () => {
				throw new Error('QuotaExceededError');
			},
			removeItem: () => {}
		});
		expect(() => chatDrafts.set('chan-1', 'still typed')).not.toThrow();
		expect(chatDrafts.get('chan-1')).toBe('still typed');
	});
});
