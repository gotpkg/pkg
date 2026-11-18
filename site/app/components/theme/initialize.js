(() => {
    const storageKey = "starlight-theme";
    const systemTheme = window.matchMedia("(prefers-color-scheme: dark)");
    let preference = "auto";

    try {
        const saved = localStorage.getItem(storageKey);
        if (saved === "light" || saved === "dark") preference = saved;
    } catch {
        // Storage may be blocked; the system preference still works.
    }

    function applyTheme() {
        const theme = preference === "auto" ? (systemTheme.matches ? "dark" : "light") : preference;
        document.documentElement.dataset.theme = theme;
        document
            .querySelector('meta[name="theme-color"]')
            ?.setAttribute("content", theme === "dark" ? "#161619" : "#f7f5ed");
        const picker = document.querySelector("[data-theme-select]");
        if (picker instanceof HTMLSelectElement) {
            picker.value = preference;
            picker.disabled = false;
        }
    }

    applyTheme();
    document.addEventListener("DOMContentLoaded", applyTheme, { once: true });
    systemTheme.addEventListener("change", () => {
        if (preference === "auto") applyTheme();
    });
    document.addEventListener("change", event => {
        const picker = event.target;
        if (!(picker instanceof HTMLSelectElement) || !picker.hasAttribute("data-theme-select"))
            return;
        preference = picker.value === "light" || picker.value === "dark" ? picker.value : "auto";
        applyTheme();
        try {
            if (preference === "auto") localStorage.removeItem(storageKey);
            else localStorage.setItem(storageKey, preference);
        } catch {
            // Keep the current-page choice when persistence is unavailable.
        }
    });
})();
