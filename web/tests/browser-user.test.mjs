import assert from 'node:assert/strict';
import test from 'node:test';
import { browserUserId } from '../src/lib/browser-user.ts';

test('browser UUID survives a module reload and replaces malformed values', async () => {
  const values = new Map();
  globalThis.window = { localStorage: { getItem: key => values.get(key), setItem: (key, value) => values.set(key, value) } };
  values.set('metabook.user-id', 'bad-id');
  const id = browserUserId();
  assert.match(id, /^[0-9a-f-]{36}$/);
  assert.equal(values.get('metabook.user-id'), id);
  const reloaded = await import('../src/lib/browser-user.ts?reload');
  assert.equal(reloaded.browserUserId(), id);
  delete globalThis.window;
});

test('blocked localStorage has a stable page-session UUID', () => {
  globalThis.window = { get localStorage() { throw new Error('blocked'); } };
  const id = browserUserId();
  assert.equal(browserUserId(), id);
  delete globalThis.window;
});
