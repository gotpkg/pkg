import { expect, test } from "@playwright/test";

const docsRoutes = [
    "/docs",
    "/docs/install",
    "/docs/setup",
    "/docs/packages",
    "/docs/configuration",
    "/docs/automation",
    "/docs/reference",
];

test("a package selection copies the corresponding command", async ({ page, context }) => {
    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    await page.goto("/");
    await page.getByRole("button", { name: "Prettier npm package" }).click();
    await expect(page.locator(".command-line code")).toHaveText("pkg install npm:prettier");
    await page.getByRole("button", { name: "Copy command" }).click();
    await expect(page.getByRole("status")).toHaveText("Command copied to clipboard.");
    await expect(page.getByText("Copied!", { exact: true })).toBeVisible();
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
        "pkg install npm:prettier",
    );
    await page.getByRole("button", { name: "IINA Mac app" }).click();
    await expect(page.getByRole("status")).toBeEmpty();
    await expect(page.locator(".command-line code")).toHaveText("pkg install brew-cask:iina");
    await page.getByRole("button", { name: "Copy command" }).click();
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
        "pkg install brew-cask:iina",
    );
});

test("clipboard denial leaves the command available and explains recovery", async ({ page }) => {
    await page.addInitScript(() => {
        Object.defineProperty(navigator.clipboard, "writeText", {
            value: () => Promise.reject(new Error("Clipboard access denied")),
        });
    });
    await page.goto("/");
    await page.getByRole("button", { name: "Copy command" }).click();
    await expect(page.getByRole("status")).toHaveText(
        "Couldn’t copy. Select and copy the command instead.",
    );
    await expect(page.locator(".command-line code")).toHaveText("pkg install brew:wget");
});

test("installation remains reachable without JavaScript", async ({ browser, baseURL }) => {
    const context = await browser.newContext({ javaScriptEnabled: false, baseURL });
    const page = await context.newPage();
    await page.goto("/");
    await expect(page.getByRole("button", { name: "Copy command" })).toBeDisabled();
    await expect(page.getByRole("button", { name: "Prettier npm package" })).toBeDisabled();
    await page.getByRole("link", { name: "Get pkg", exact: false }).click();
    await expect(page).toHaveURL(/\/docs\/install\/?$/);
    await expect(page.getByRole("main")).toContainText("curl -fsSL https://gotpkg.dev | sh");
    await context.close();
});

test("mobile pages fit the viewport and every local content link resolves", async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 812 });
    const checked = new Set<string>();
    for (const route of ["/", ...docsRoutes]) {
        await page.goto(route);
        expect(
            await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
        ).toBe(true);
        const links = await page
            .locator("main a[href]")
            .evaluateAll(elements => elements.map(element => (element as HTMLAnchorElement).href));
        for (const href of links) {
            const url = new URL(href);
            if (url.origin !== new URL(page.url()).origin || checked.has(href)) continue;
            checked.add(href);
            const response = await page.request.get(url.href);
            expect(response.ok(), href).toBe(true);
            if (url.hash) {
                const html = await response.text();
                expect(html, href).toContain(`id="${decodeURIComponent(url.hash.slice(1))}"`);
            }
        }
    }
});

test("documentation search finds a package workflow", async ({ page }) => {
    await page.goto("/docs");
    await page.getByRole("button", { name: /Search/ }).click();
    await page.getByRole("textbox", { name: "Search", exact: true }).fill("dry-run");
    await expect(page.locator(".pagefind-ui__result-link").first()).toBeVisible();
    await page.locator(".pagefind-ui__result-link").first().click();
    await expect(page).toHaveURL(/\/docs\//);
});
