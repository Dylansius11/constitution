import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

const root = fileURLToPath(new URL('../scratch/', import.meta.url));
const launcher = new URL('./run-service.mjs', import.meta.url).href;
const linuxOnly = { skip: process.platform !== 'linux' && 'Linux service lifecycle tests require /proc and POSIX signals', timeout: 20_000 };

async function waitFor(predicate) {
  const deadline = Date.now() + 5_000;
  while (!(await predicate())) {
    assert.ok(Date.now() < deadline, 'timed out waiting for child fixture');
    await delay(20);
  }
}

async function exists(path) {
  try { await readFile(path); return true; }
  catch (error) { if (error.code === 'ENOENT') return false; throw error; }
}

async function fixtureProcesses(directory) {
  const pids = [];
  for (const entry of await readdir('/proc')) {
    if (!/^\d+$/.test(entry)) continue;
    try {
      const argv = (await readFile(`/proc/${entry}/cmdline`, 'utf8')).split('\0');
      if (argv.some(arg => arg.startsWith(directory + '/') && arg.endsWith('.mjs'))) pids.push(Number(entry));
    } catch (error) {
      if (!['ENOENT', 'ESRCH', 'EACCES'].includes(error.code)) throw error;
    }
  }
  return pids;
}

async function fixture(t, { missing, stubborn = false, args = [] } = {}) {
  await mkdir(root, { recursive: true });
  const directory = await mkdtemp(join(root, 'service-lifecycle-'));
  const childSource = `
import { existsSync, writeFileSync } from 'node:fs';
const name = process.argv[2];
for (const signal of ['SIGTERM', 'SIGINT']) process.on(signal, () => {
  writeFileSync(name + '.signal', signal);
  if (!${stubborn}) setTimeout(() => process.exit(0), 150);
});
writeFileSync(name + '.pid', String(process.pid));
setInterval(() => {
  if (existsSync(name + '.exit')) process.exit(Number(process.argv[3]));
}, 20);
`;
  await writeFile(join(directory, 'child.mjs'), childSource);
  const command = name => missing === name
    ? [join(directory, 'nonexistent-executable')]
    : [process.execPath, join(directory, 'child.mjs'), name, '0'];
  await writeFile(join(directory, 'supervisor.mjs'), `
import { runService } from ${JSON.stringify(launcher)};
process.exitCode = await runService({
  args: process.argv.slice(2),
  cwd: ${JSON.stringify(directory)},
  verifier: ${JSON.stringify(command('verifier'))},
  sidecar: ${JSON.stringify(command('sidecar'))},
});
`);
  const child = spawn(process.execPath, [join(directory, 'supervisor.mjs'), ...args], { stdio: 'ignore' });
  const closed = new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('close', (code, signal) => resolve({ code, signal }));
  });
  t.after(async () => {
    child.kill('SIGKILL');
    await closed;
    for (const pid of await fixtureProcesses(directory)) {
      try { process.kill(pid, 'SIGKILL'); } catch (error) { if (error.code !== 'ESRCH') throw error; }
    }
    await rm(directory, { recursive: true, force: true });
  });
  return {
    child, closed, directory,
    ready: () => waitFor(async () => (await exists(join(directory, 'verifier.pid'))) && (await exists(join(directory, 'sidecar.pid')))),
    async assertReaped() {
      for (const name of ['verifier', 'sidecar']) {
        if (await exists(join(directory, name + '.pid'))) {
          const pid = Number(await readFile(join(directory, name + '.pid'), 'utf8'));
          assert.throws(() => process.kill(pid, 0), { code: 'ESRCH' }, `${name} must be reaped before supervisor exit`);
        }
      }
      assert.deepEqual(await fixtureProcesses(directory), [], 'no fixture subprocess may survive supervisor exit');
    },
  };
}

// Removing either exit listener or treating status zero as success leaves the sibling alive.
for (const name of ['verifier', 'sidecar']) {
  test(`${name} unexpected clean exit fails service and reaps sibling`, linuxOnly, async t => {
    const run = await fixture(t);
    await run.ready();
    await writeFile(join(run.directory, name + '.exit'), '');
    const result = await run.closed;
    assert.equal(result.signal, null);
    assert.equal(result.code, 1);
    const sibling = name === 'verifier' ? 'sidecar' : 'verifier';
    assert.equal(await readFile(join(run.directory, sibling + '.signal'), 'utf8'), 'SIGTERM');
    await run.assertReaped();
  });
}

// Not forwarding a signal, exiting before close, or using status zero breaks these cases.
for (const [signal, code] of [['SIGTERM', 143], ['SIGINT', 130]]) {
  test(`${signal} stops and reaps both children with conventional status`, linuxOnly, async t => {
    const run = await fixture(t, { args: ['--insecure-dev-identity'] });
    await run.ready();
    run.child.kill(signal);
    assert.deepEqual(await run.closed, { code, signal: null });
    for (const name of ['verifier', 'sidecar']) {
      assert.equal(await readFile(join(run.directory, name + '.signal'), 'utf8'), signal);
    }
    await run.assertReaped();
  });
}

// Ignoring spawn errors or abandoning a sibling leaves a real process behind.
for (const missing of ['verifier', 'sidecar']) {
  test(`${missing} spawn failure fails service without orphaning sibling`, linuxOnly, async t => {
    const run = await fixture(t, { missing });
    assert.deepEqual(await run.closed, { code: 1, signal: null });
    await run.assertReaped();
  });
}

test('children ignoring termination are killed after bounded ten-second grace', linuxOnly, async t => {
  const run = await fixture(t, { stubborn: true });
  await run.ready();
  const started = Date.now();
  run.child.kill('SIGTERM');
  assert.deepEqual(await run.closed, { code: 143, signal: null });
  const elapsed = Date.now() - started;
  assert.ok(elapsed >= 9_500 && elapsed < 15_000, 'shutdown must escalate at ten seconds, not hang or kill immediately');
  await run.assertReaped();
});

// Validation after spawn creates observable child startup even for invalid invocations.
for (const args of [['--other'], ['--insecure-dev-identity', '--other'], ['--insecure-dev-identity', '--insecure-dev-identity']]) {
  test(`invalid arguments ${JSON.stringify(args)} fail before children start`, linuxOnly, async t => {
    const run = await fixture(t, { args });
    assert.deepEqual(await run.closed, { code: 2, signal: null });
    assert.equal(await exists(join(run.directory, 'verifier.pid')), false);
    assert.equal(await exists(join(run.directory, 'sidecar.pid')), false);
    await run.assertReaped();
  });
}
