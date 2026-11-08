import test from 'node:test';
import assert from 'node:assert/strict';

// Offline dependency compatibility check: constructing this SDK is lazy and does
// not send attestation, completion, or signature requests.
test('pinned Node SDK imports and accepts strict client configuration offline', { timeout: 120_000 }, async () => {
  const { InferenceClient } = await import('@nearai/inference-sdk/node');
  const strict = { policy: { acceptedTcbStatuses: ['UpToDate'] } };
  const client = new InferenceClient({ apiKey: 'local-noncredential', baseUrl: 'https://cloud-api.near.ai/v1', e2ee: true, gatewayVerification: strict, modelVerification: strict });
  assert.equal(client.getBaseUrl().replace(/\/$/, ''), 'https://cloud-api.near.ai/v1');
});
