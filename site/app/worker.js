import { handle } from "@astrojs/cloudflare/handler";

const INSTALLER = "https://github.com/gotpkg/pkg/releases/latest/download/install.sh";
const MAX_AGE_SECONDS = 300;

export default {
    async fetch(request, env, context) {
        const { pathname } = new URL(request.url);
        if (pathname === "/install.sh") return installer();
        if (pathname !== "/") return handle(request, env, context);

        const acceptsHtml = (request.headers.get("Accept") ?? "").includes("text/html");
        const response = acceptsHtml ? await handle(request, env, context) : await installer();
        const varied = new Response(response.body, response);
        varied.headers.append("Vary", "Accept");
        return varied;
    },
};

async function installer() {
    let release;
    try {
        release = await fetch(INSTALLER, {
            cf: { cacheTtl: MAX_AGE_SECONDS, cacheEverything: true },
        });
    } catch {
        return text("pkg's installer is not available: could not reach GitHub.\n", 502, "no-store");
    }
    if (!release.ok) {
        return text(
            `pkg's installer is not available: GitHub answered ${release.status}.\n`,
            502,
            "no-store",
        );
    }
    return text(release.body, 200, `public, max-age=${MAX_AGE_SECONDS}`);
}

function text(body, status, cacheControl) {
    return new Response(body, {
        status,
        headers: { "Content-Type": "text/plain; charset=utf-8", "Cache-Control": cacheControl },
    });
}
