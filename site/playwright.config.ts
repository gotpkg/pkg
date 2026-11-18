import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
    testDir: "e2e",
    forbidOnly: !!process.env.CI,
    reporter: "list",
    use: { baseURL: process.env.PREVIEW_URL, trace: "retain-on-failure" },
    projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
    webServer: {
        command: "astro preview --host 127.0.0.1 --port 0 --ignore-lock",
        wait: { stdout: /(?<preview_url>http:\/\/127\.0\.0\.1:\d+)/ },
        stdout: "ignore",
        timeout: 60_000,
    },
});
