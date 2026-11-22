import { expect, test } from "@playwright/test";

test("both Worker-served and static HTML identify the build being deployed", async ({
    request,
}) => {
    const expectedBuild = (process.env.GITHUB_SHA ?? "dev").slice(0, 7);
    for (const path of ["/", "/docs/install"]) {
        const response = await request.get(path, { headers: { Accept: "text/html" } });
        expect(response.status()).toBe(200);
        expect(response.headers()["content-type"]).toMatch(/^text\/html/);
        expect(response.headers()["x-build"]).toBe(expectedBuild);
    }
});
