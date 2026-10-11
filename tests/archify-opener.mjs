// Consumer regression checks live outside the catalog-managed package so an
// ordinary Lexicon update preserves both source provenance and this coverage.
import assert from 'node:assert/strict';
import test from 'node:test';
import { openLoopbackUrl } from '../.agents/skills/archify/bin/open-artifact.mjs';

const target = 'http://127.0.0.1:1234/';

for (const [reason, url] of [
  ['invalid URL', 'not a URL'],
  ['HTTPS', 'https://127.0.0.1:1234/'],
  ['file scheme', 'file:///tmp/preview.html'],
  ['localhost name', 'http://localhost:1234/'],
  ['remote host', 'http://example.com:1234/'],
  ['missing port', 'http://127.0.0.1/'],
  ['username', 'http://user@127.0.0.1:1234/'],
  ['password', 'http://:secret@127.0.0.1:1234/'],
  ['non-root path', 'http://127.0.0.1:1234/artifact.html'],
  ['query', 'http://127.0.0.1:1234/?reload=1'],
  ['fragment', 'http://127.0.0.1:1234/#preview'],
]) {
  test(`rejects ${reason} before launching any process`, () => {
    let calls = 0;
    assert.throws(() => openLoopbackUrl(url, {
      platform: 'linux',
      spawn() { calls += 1; return { status: 0 }; },
    }), TypeError);
    assert.equal(calls, 0);
  });
}

for (const [platform, command, method, args] of [
  ['linux', 'xdg-open', 'xdg-open', [target]],
  ['darwin', 'open', 'open', [target]],
  ['win32', 'powershell.exe', 'powershell', [
    '-NoProfile', '-NonInteractive', '-Command',
    'Start-Process -FilePath $args[0]', target,
  ]],
]) {
  test(`opens a validated loopback root on ${platform} without a shell`, () => {
    const calls = [];
    const result = openLoopbackUrl(target, {
      platform,
      spawn(...argv) { calls.push(argv); return { status: 0 }; },
    });
    assert.deepEqual(result, { requested: true, status: 'opened', target, method });
    assert.equal(calls.length, 1);
    assert.equal(calls[0][0], command);
    assert.deepEqual(calls[0][1], args);
    assert.equal(calls[0][2].shell, false);
    assert.equal(calls[0][2].timeout, 5000);
  });
}

for (const [reason, spawn, status] of [
  ['missing executable', () => ({ error: { code: 'ENOENT' } }), 'unsupported'],
  ['permission error', () => ({ error: { code: 'EACCES' } }), 'failed'],
  ['thrown spawn', () => { throw new Error('cannot spawn'); }, 'failed'],
  ['nonzero exit', () => ({ status: 1 }), 'failed'],
  ['signal', () => ({ status: 0, signal: 'SIGTERM' }), 'failed'],
  ['missing result', () => undefined, 'failed'],
]) {
  test(`reports ${reason} truthfully`, () => {
    assert.deepEqual(openLoopbackUrl(target, { platform: 'linux', spawn }), {
      requested: true, status, target, method: 'xdg-open',
    });
  });
}

test('reports an unsupported platform without launching a process', () => {
  let calls = 0;
  assert.deepEqual(openLoopbackUrl(target, {
    platform: 'unsupported-test-platform',
    spawn() { calls += 1; return { status: 0 }; },
  }), { requested: true, status: 'unsupported', target, method: null });
  assert.equal(calls, 0);
});
