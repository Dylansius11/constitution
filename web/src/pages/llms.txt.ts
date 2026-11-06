import { buildLlmsText } from "../data/llms.ts";
import { site } from "../data/site.ts";

export const prerender = true;

export const GET = (): Response =>
  new Response(buildLlmsText(site), {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
