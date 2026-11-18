import assert from "node:assert/strict";
import { test } from "node:test";

import { route } from "./worker.js";

const INSTALLER = "https://github.com/gotpkg/pkg/releases/latest/download/install.sh";
const SCRIPT = '#!/bin/sh\nrelease_version="1.2.3"\n';
const BROWSER = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";

async function github(url) {
    assert.equal(url, INSTALLER);
    return new Response(SCRIPT);
}

async function noRelease() {
    return new Response("Not Found", { status: 404 });
}

function get(path, accept) {
    return new Request(`https://gotpkg.dev${path}`, { headers: { Accept: accept } });
}

function maxAge(response) {
    return Number(/max-age=(\d+)/.exec(response.headers.get("Cache-Control"))[1]);
}

test("/install.sh is the latest release's installer, even for a browser", async () => {
    const response = await route(get("/install.sh", BROWSER), github);
    assert.equal(response.status, 200);
    assert.match(response.headers.get("Content-Type"), /^text\/plain/);
    assert.equal(await response.text(), SCRIPT);
    assert.ok(maxAge(response) <= 300);
});

test("the root gives curl the installer", async () => {
    const response = await route(get("/", "*/*"), github);
    assert.equal(response.status, 200);
    assert.equal(await response.text(), SCRIPT);
    assert.equal(response.headers.get("Vary"), "Accept");
    assert.ok(maxAge(response) <= 300);
});

test("the root gives a browser a page that shows the install command", async () => {
    const response = await route(get("/", BROWSER), noRelease);
    assert.equal(response.status, 200);
    assert.match(response.headers.get("Content-Type"), /^text\/html/);
    assert.equal(response.headers.get("Vary"), "Accept");
    assert.match(await response.text(), /curl -fsSL https:\/\/gotpkg\.dev \| sh/);
});

test("without a release, the installer routes fail instead of serving something else", async () => {
    for (const path of ["/", "/install.sh"]) {
        const response = await route(get(path, "*/*"), noRelease);
        assert.equal(response.status, 502);
        assert.equal(response.headers.get("Cache-Control"), "no-store");
    }
});

test("other paths are not found", async () => {
    const response = await route(get("/docs", BROWSER), github);
    assert.equal(response.status, 404);
});
