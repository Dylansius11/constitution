import { defineConfig } from "astro/config";

// Static output only (ARCHITECTURE: Astro static page). The host (D8) is undecided, so nothing here assumes server features.
export default defineConfig({
  output: "static",
});
