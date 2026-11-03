import { createServer } from './review.mjs';
// Fixed address: neither environment nor requests can widen the listener.
createServer().listen(8788, '127.0.0.1');
