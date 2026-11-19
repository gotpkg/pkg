import { expect, test } from "@playwright/test";

test.use({ javaScriptEnabled: false });

test("HTML sharing metadata identifies each page and a downloadable PNG without JavaScript", async ({
    page,
}) => {
    const titles = new Set<string>();
    for (const path of ["/", "/docs/install"]) {
        await page.goto(`${path}?utm_source=sharing`);
        const canonical = new URL(path, "https://gotpkg.dev").href;
        await expect(page.locator('link[rel="canonical"]')).toHaveAttribute("href", canonical);
        await expect(page.locator('meta[property="og:url"]')).toHaveAttribute("content", canonical);
        for (const property of ["og:title", "og:description", "og:image:alt"]) {
            await expect(page.locator(`meta[property="${property}"]`)).toHaveAttribute(
                "content",
                /\S/,
            );
        }
        titles.add((await page.locator('meta[property="og:title"]').getAttribute("content"))!);
        const imageUrl = "https://gotpkg.dev/og-image.png";
        await expect(page.locator('meta[property="og:image"]')).toHaveAttribute(
            "content",
            imageUrl,
        );
        await expect(page.locator('meta[name="twitter:image"]')).toHaveAttribute(
            "content",
            imageUrl,
        );
        await expect(page.locator('meta[name="twitter:card"]')).toHaveAttribute(
            "content",
            "summary_large_image",
        );
        const image = await page.request.get(new URL(imageUrl).pathname);
        expect(image.status()).toBe(200);
        expect(image.headers()["content-type"]).toMatch(/^image\/png/);
        const bytes = await image.body();
        expect(bytes.subarray(0, 8)).toEqual(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
        await expect(page.locator('meta[property="og:image:width"]')).toHaveAttribute(
            "content",
            String(bytes.readUInt32BE(16)),
        );
        await expect(page.locator('meta[property="og:image:height"]')).toHaveAttribute(
            "content",
            String(bytes.readUInt32BE(20)),
        );
    }
    expect(titles.size).toBe(2);
});
