// Serves gotpkg.dev until the site's Worker takes it over: the latest release's installer for
// `curl -fsSL https://gotpkg.dev | sh` and `/install.sh`, and a placeholder page otherwise.

import { ROOT_VARIATION, wantsInstaller } from "../../site/app/installer-routing.js";

const INSTALLER = "https://github.com/gotpkg/pkg/releases/latest/download/install.sh";

// The release workflow waits five minutes for gotpkg.dev to serve a new release's installer.
const MAX_AGE_SECONDS = 300;

export default {
    fetch(request) {
        return route(request, fetch);
    },
};

/** The response to `request`, getting the installer with `fetchInstaller`. */
export async function route(request, fetchInstaller) {
    const { pathname } = new URL(request.url);
    if (pathname === "/install.sh") {
        return installer(fetchInstaller);
    }
    if (pathname !== "/") {
        return text("Not found\n", 404, "no-store");
    }
    const response = wantsInstaller(request.headers) ? await installer(fetchInstaller) : page();
    response.headers.append("Vary", ROOT_VARIATION);
    return response;
}

async function installer(fetchInstaller) {
    const release = await fetchInstaller(INSTALLER, {
        cf: { cacheTtl: MAX_AGE_SECONDS, cacheEverything: true },
    });
    if (!release.ok) {
        const message = `pkg's installer is not available: GitHub answered ${release.status} for ${INSTALLER}\n`;
        return text(message, 502, "no-store");
    }
    return text(release.body, 200, `public, max-age=${MAX_AGE_SECONDS}`);
}

function text(body, status, cacheControl) {
    return new Response(body, {
        status,
        headers: { "Content-Type": "text/plain; charset=utf-8", "Cache-Control": cacheControl },
    });
}

function page() {
    return new Response(PAGE, {
        headers: {
            "Content-Type": "text/html; charset=utf-8",
            "Cache-Control": `public, max-age=${MAX_AGE_SECONDS}`,
        },
    });
}

const PAGE = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>pkg: the unified system package manager</title>
</head>
<body>
<main>
<h1>pkg</h1>
<p>The unified system package manager.</p>
<pre><code>curl -fsSL https://gotpkg.dev | sh</code></pre>
<p><a href="https://github.com/gotpkg/pkg">github.com/gotpkg/pkg</a></p>
</main>
</body>
</html>
`;
