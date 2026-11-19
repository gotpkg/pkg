import assert from "node:assert/strict";
import { globSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { test, type TestContext } from "node:test";

import { convertV4MiniflareOptions, Miniflare, type Response as WorkerResponse } from "miniflare";
import { unstable_getMiniflareWorkerOptions } from "wrangler";

const INSTALLER = "https://github.com/gotpkg/pkg/releases/latest/download/install.sh";
const SCRIPT = '#!/bin/sh\nrelease_version="1.2.3"\n';
const BROWSER = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";

function runtime(context: TestContext, upstreamStatus = 200) {
    const pointerPath = resolve(".wrangler/deploy/config.json");
    const pointer = JSON.parse(readFileSync(pointerPath, "utf8")) as { configPath: string };
    const configPath = resolve(dirname(pointerPath), pointer.configPath);
    const { main, workerOptions, externalWorkers } = unstable_getMiniflareWorkerOptions(configPath);
    assert.ok(main, "the deployment must include the installer Worker");
    // The built ESM manifest replaces Wrangler's source-file rules.
    delete workerOptions.modulesRules;
    const upstreamRequests: string[] = [];
    const worker = new Miniflare(
        convertV4MiniflareOptions({
            cf: false,
            workers: [
                {
                    ...workerOptions,
                    modulesRoot: dirname(main),
                    modules: builtModules(main),
                    outboundService(request) {
                        upstreamRequests.push(request.url);
                        assert.equal(request.url, INSTALLER);
                        return new Response(upstreamStatus === 200 ? SCRIPT : "Unavailable", {
                            status: upstreamStatus,
                        });
                    },
                },
                ...externalWorkers,
            ],
        }),
    );
    context.after(() => worker.dispose());
    return { worker, upstreamRequests };
}

function builtModules(main: string) {
    const directory = dirname(main);
    const paths = [
        main,
        ...globSync(["**/*.js", "**/*.mjs"], { cwd: directory })
            .map(path => resolve(directory, path))
            .filter(path => path !== main),
    ];
    return paths.map(path => ({ type: "ESModule" as const, path }));
}

function installerResponse(response: WorkerResponse) {
    assert.equal(response.status, 200);
    assert.match(response.headers.get("Content-Type") ?? "", /^text\/plain/);
    const maxAge = /(?:^|,)\s*max-age=(\d+)/.exec(response.headers.get("Cache-Control") ?? "");
    assert.ok(maxAge, "installer responses declare a cache lifetime");
    assert.ok(Number(maxAge[1]) <= 300);
}

await test("HTML root requests render the site without fetching an installer", async context => {
    const { worker, upstreamRequests } = runtime(context, 404);
    const response = await worker.dispatchFetch("https://gotpkg.dev/", {
        headers: { Accept: BROWSER, "User-Agent": "curl" },
    });
    assert.equal(response.status, 200);
    assert.match(response.headers.get("Content-Type") ?? "", /^text\/html/);
    assert.match(response.headers.get("Vary") ?? "", /(?:^|,)\s*Accept(?:,|$)/i);
    assert.match(await response.text(), /href="\/docs\/install"/);
    assert.deepEqual(upstreamRequests, []);
});

await test("non-HTML and absent Accept headers receive the latest installer at the root", async context => {
    const { worker, upstreamRequests } = runtime(context);
    for (const headers of [{ Accept: "*/*", "User-Agent": "Mozilla/5.0" }, {}]) {
        const response = await worker.dispatchFetch("https://gotpkg.dev/", { headers });
        installerResponse(response);
        assert.match(response.headers.get("Vary") ?? "", /(?:^|,)\s*Accept(?:,|$)/i);
        assert.equal(await response.text(), SCRIPT);
    }
    assert.ok(upstreamRequests.every(url => url === INSTALLER));
});

await test("the explicit installer route returns the script even to a browser", async context => {
    const { worker } = runtime(context);
    const response = await worker.dispatchFetch("https://gotpkg.dev/install.sh", {
        headers: { Accept: BROWSER },
    });
    installerResponse(response);
    assert.equal(await response.text(), SCRIPT);
});

await test("unavailable releases fail without caching or substituting a script", async context => {
    const { worker } = runtime(context, 404);
    for (const path of ["/", "/install.sh"]) {
        const response = await worker.dispatchFetch(`https://gotpkg.dev${path}`, {
            headers: { Accept: "*/*" },
        });
        assert.equal(response.status, 502);
        assert.equal(response.headers.get("Cache-Control"), "no-store");
        assert.match(await response.text(), /installer.*not available/i);
    }
});

await test("other asset paths retain their normal behavior for non-HTML clients", async context => {
    const { worker, upstreamRequests } = runtime(context);
    const response = await worker.dispatchFetch("https://gotpkg.dev/docs/install", {
        headers: { Accept: "*/*" },
    });
    assert.equal(response.status, 200);
    assert.match(response.headers.get("Content-Type") ?? "", /^text\/html/);
    assert.notEqual(await response.text(), SCRIPT);
    assert.deepEqual(upstreamRequests, []);
});
