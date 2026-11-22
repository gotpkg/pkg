import { appendFile } from "node:fs/promises";

import type { AstroIntegration } from "astro";

// Static pages bypass Worker code, so their build stamp belongs in Cloudflare's asset headers.
export function stampBuild(sha: string): AstroIntegration {
    return {
        name: "stamp-build",
        hooks: {
            // With `output: "server"`, `dir` is the client output (dist/client), not dist/.
            "astro:build:done": async ({ dir }) => {
                await appendFile(new URL("_headers", dir), `\n/*\n  X-Build: ${sha}\n`);
            },
        },
    };
}
