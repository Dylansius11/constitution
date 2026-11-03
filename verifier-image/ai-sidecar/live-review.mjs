import { randomBytes } from 'node:crypto';
import { review } from './review.mjs';
// Opt-in, potentially billable. Run only with founder-authorized B4 credentials.
const result = await review({ files: [{ path: 'src/Probe.sol', content: '// Verification probe only; no vault implementation is supplied.' }], rules: ['No executable vault is supplied; return {"candidates":[]}.'], nonce: randomBytes(32).toString('hex'), max_candidates: 1, timeout_s: 120 });
console.log(JSON.stringify(result));
if (result.status !== 'complete' || result.signatureKind !== 'provider_tee') process.exitCode = 1;
