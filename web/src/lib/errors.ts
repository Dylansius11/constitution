/** Untrusted data (wire JSON, bytes, hex) is malformed. `path` names the offending field. */
export class WireError extends Error {
  readonly path: string;
  constructor(path: string, message: string) {
    super(`${path}: ${message}`);
    this.name = "WireError";
    this.path = path;
  }
}

/** A caller of this library passed an argument that can never be valid. Nothing was sent. */
export class InputError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "InputError";
  }
}

/** The request never produced an HTTP response: DNS, offline, CORS, timeout or abort. */
export class NetworkError extends Error {
  constructor(message: string, options?: { cause: unknown }) {
    super(message, options);
    this.name = "NetworkError";
  }
}

/** The server answered with a non-success HTTP status. `body` is a bounded prefix of the response text. */
export class HttpStatusError extends Error {
  readonly status: number;
  readonly body: string;
  constructor(status: number, body: string) {
    super(`HTTP ${status}${body ? `: ${body}` : ""}`);
    this.name = "HttpStatusError";
    this.status = status;
    this.body = body;
  }
}

/** A JSON-RPC server reported an error object, or a contract view failed. */
export class RpcError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "RpcError";
  }
}
