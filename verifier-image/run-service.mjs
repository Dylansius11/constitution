import { spawn } from 'node:child_process';
import { pathToFileURL } from 'node:url';

// Command overrides allow the same supervisor to run real local subprocesses.
// The container entrypoint always uses these fixed production defaults.
export async function runService({
  args = process.argv.slice(2),
  env = process.env,
  cwd = '/opt/constitution',
  verifier = ['/usr/local/bin/constitution-verifier'],
  sidecar = [process.execPath, '/opt/constitution/verifier-image/ai-sidecar/server.mjs'],
} = {}) {
  if (args.length !== 0 && !(args.length === 1 && args[0] === '--insecure-dev-identity')) {
    console.error('Usage: run-service.mjs [--insecure-dev-identity]');
    return 2;
  }

  const verifierEnv = {};
  const sidecarEnv = {};
  for (const [name, value] of Object.entries(env)) {
    if (name.startsWith('NEARAI_')) sidecarEnv[name] = value;
    else verifierEnv[name] = value;
    if (name === 'PATH' || name === 'HOME') sidecarEnv[name] = value;
  }

  return new Promise(resolve => {
    const children = new Set();
    let stopping = false;
    let exitCode;
    let timer;

    const onTerm = () => stop(143, 'SIGTERM');
    const onInt = () => stop(130, 'SIGINT');
    function finish() {
      if (!stopping || children.size !== 0) return;
      clearTimeout(timer);
      process.removeListener('SIGTERM', onTerm);
      process.removeListener('SIGINT', onInt);
      resolve(exitCode);
    }
    function stop(code, signal = 'SIGTERM') {
      if (stopping) return;
      stopping = true;
      exitCode = code;
      timer = setTimeout(() => {
        for (const child of children) child.kill('SIGKILL');
      }, 10_000);
      for (const child of children) child.kill(signal);
      finish();
    }

    process.on('SIGTERM', onTerm);
    process.on('SIGINT', onInt);
    for (const [name, command, childArgs, childEnv] of [
      ['verifier', verifier[0], [...verifier.slice(1), ...args], verifierEnv],
      ['sidecar', sidecar[0], sidecar.slice(1), sidecarEnv],
    ]) {
      try {
        const child = spawn(command, childArgs, { cwd, env: childEnv, stdio: ['ignore', 'inherit', 'inherit'] });
        children.add(child);
        child.on('error', () => {
          // Error objects can include arguments and environment-derived paths.
          console.error(`Service ${name} process error; stopping service.`);
          stop(1);
        });
        child.once('close', () => {
          children.delete(child);
          if (!stopping) {
            console.error(`Service ${name} exited unexpectedly; stopping service.`);
            stop(1);
          } else finish();
        });
      } catch {
        console.error(`Service ${name} could not start; stopping service.`);
        stop(1);
        break;
      }
    }
  });
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = await runService();
}
