import { expect, test } from "@playwright/test";

test("the landing theme follows the system until overridden and stays in sync with docs", async ({
    page,
}) => {
    await page.emulateMedia({ colorScheme: "dark" });
    await page.goto("/");
    const picker = page.getByRole("combobox", { name: "Select theme" });
    await expect(picker).toHaveValue("auto");
    await expect(page.locator("html")).toHaveCSS("color-scheme", "dark");

    await page.emulateMedia({ colorScheme: "light" });
    await expect(page.locator("html")).toHaveCSS("color-scheme", "light");
    await picker.selectOption("dark");
    await page.reload();
    await expect(picker).toHaveValue("dark");
    await expect(page.locator("html")).toHaveCSS("color-scheme", "dark");

    await page
        .getByRole("navigation", { name: "Main navigation" })
        .getByRole("link", { name: "Docs", exact: true })
        .click();
    await expect(picker).toHaveValue("dark");
    await picker.selectOption("light");
    await page.getByRole("link", { name: "pkg", exact: true }).click();
    await expect(picker).toHaveValue("light");
    await expect(page.locator("html")).toHaveCSS("color-scheme", "light");
    await page.emulateMedia({ colorScheme: "dark" });
    await expect(page.locator("html")).toHaveCSS("color-scheme", "light");
    await picker.selectOption("auto");
    await expect(page.locator("html")).toHaveCSS("color-scheme", "dark");
    await page.reload();
    await expect(picker).toHaveValue("auto");
});

test("theme switching works for the current page when storage is blocked", async ({ page }) => {
    await page.addInitScript(() => {
        Object.defineProperty(window, "localStorage", {
            get() {
                throw new DOMException("Storage unavailable", "SecurityError");
            },
        });
    });
    await page.emulateMedia({ colorScheme: "light" });
    await page.goto("/");
    await page.getByRole("combobox", { name: "Select theme" }).selectOption("dark");
    await expect(page.locator("html")).toHaveCSS("color-scheme", "dark");
    await page.getByRole("link", { name: "Get pkg", exact: false }).click();
    await expect(page).toHaveURL(/\/docs\/install\/?$/);
});
