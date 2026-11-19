import { expect, test } from "@playwright/test";

test("the landing follows the system independently of the saved documentation theme", async ({
    page,
}) => {
    const background = () =>
        page.locator("html").evaluate(element => getComputedStyle(element).backgroundColor);
    await page.emulateMedia({ colorScheme: "dark" });
    await page.goto("/");
    const darkBackground = await background();

    await page.emulateMedia({ colorScheme: "light" });
    await expect.poll(background).not.toBe(darkBackground);
    await page.emulateMedia({ colorScheme: "dark" });
    await expect.poll(background).toBe(darkBackground);

    await page
        .getByRole("navigation", { name: "Main navigation" })
        .getByRole("link", { name: "Docs", exact: true })
        .click();
    const picker = page.getByRole("combobox", { name: "Select theme" });
    await picker.selectOption("light");
    await page.reload();
    await expect(picker).toHaveValue("light");
    await expect(page.locator("html")).toHaveCSS("color-scheme", "light");
    await page.getByRole("link", { name: "pkg", exact: true }).click();
    await expect.poll(background).toBe(darkBackground);
});

test("the landing follows system changes when storage is blocked", async ({ page }) => {
    await page.addInitScript(() => {
        Object.defineProperty(window, "localStorage", {
            get() {
                throw new DOMException("Storage unavailable", "SecurityError");
            },
        });
    });
    await page.emulateMedia({ colorScheme: "light" });
    await page.goto("/");
    const background = () =>
        page.locator("html").evaluate(element => getComputedStyle(element).backgroundColor);
    const lightBackground = await background();
    await page.emulateMedia({ colorScheme: "dark" });
    await expect.poll(background).not.toBe(lightBackground);
    await page.getByRole("link", { name: "Get pkg", exact: false }).click();
    await expect(page).toHaveURL(/\/docs\/install\/?$/);
});
