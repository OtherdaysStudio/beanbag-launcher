// Walks every screen of the built app with the keyboard map and fails on a broken step
// or a console error. SHOTS=<dir> also saves a screenshot of each step.
import { spawn } from "node:child_process";
import { mkdirSync } from "node:fs";
import { chromium } from "playwright";

const PORT = 4173;
const shots = process.env.SHOTS;
if (shots) mkdirSync(shots, { recursive: true });

// npx is a .cmd file on Windows, which only a shell can start.
const server = spawn("npx", ["vite", "preview", "--port", String(PORT), "--strictPort"], {
  stdio: "ignore",
  shell: process.platform === "win32",
});
const browser = await chromium.launch();
let failed = false;

try {
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("console", (m) => {
    // Sample art comes from the Steam CDN; a missing image is a design case, not a fault.
    if (m.type() === "error" && !m.text().includes("Failed to load resource")) errors.push(m.text());
  });

  for (let i = 0; ; i++) {
    try {
      await page.goto(`http://localhost:${PORT}/`);
      break;
    } catch (e) {
      if (i > 40) throw e;
      await page.waitForTimeout(250);
    }
  }

  const step = async (name, keys, selector, check) => {
    for (const k of keys) {
      await page.keyboard.press(k);
      await page.waitForTimeout(80);
    }
    await page.waitForSelector(selector, { timeout: 3000 });
    if (check) {
      const ok = await page.evaluate(check);
      if (!ok) throw new Error(`${name}: check failed`);
    }
    await page.waitForTimeout(700);
    if (shots) await page.screenshot({ path: `${shots}/${name}.png` });
    console.log(`ok  ${name}`);
  };

  await step("1-home", [], ".tile[data-focused]");

  // The mouse must not move focus or scroll the page.
  const pageTransform = () => page.evaluate(() => document.querySelector(".home")?.style.transform ?? "");
  const before = await pageTransform();
  const poster = page.locator(".home .tile").nth(6);
  await poster.hover();
  await page.mouse.move(900, 700, { steps: 8 });
  await page.waitForTimeout(400);
  if ((await pageTransform()) !== before) throw new Error("hover scrolled the page");
  if (!(await page.locator(".tile[data-focused]").count())) throw new Error("hover moved focus");
  console.log("ok  hover leaves focus and scroll alone");

  await step("2-recent-next", ["ArrowRight"], '.tile[data-focused][aria-label="Balatro"]', () =>
    document.querySelector(".hints[data-keys]") !== null,
  );
  await step("3-library", ["ArrowDown", "ArrowRight", "ArrowRight"], '.page[data-scrolled] .shelf[aria-label="Story"] .tile[data-focused]');
  // A long row slides sideways and keeps one card showing past the focus.
  await step("3b-row-scrolls", Array(5).fill("ArrowRight"), ".tile[data-focused]", () =>
    /-\d/.test(document.querySelector(".tile[data-focused]")?.closest(".track")?.style.transform ?? ""),
  );
  await step("3c-up-keeps-screen-place", ["ArrowDown", "ArrowUp"], '.shelf[aria-label="Story"] .tile[data-focused]');
  await step("4-sheet", ["Enter"], ".sheet[data-open] .pill[data-focused]");
  // Change art: tabs for each kind, a grid of images, Change game, and back to the game page.
  await step("4b-change-art", ["ArrowRight", "Enter"], ".art .art-tile[data-focused]", () =>
    document.querySelector(".art-tab[aria-selected='true']")?.textContent?.startsWith("Cover"),
  );
  await step("4c-art-tabs", ["Tab"], ".art-tab[aria-selected='true']", () =>
    document.querySelector(".art-tab[aria-selected='true']")?.textContent?.startsWith("Background"),
  );
  await step("4d-change-game", ["/"], ".art .art-games");
  await step("4e-art-back", ["Escape", "Escape"], ".sheet[data-open] .pill[data-focused]", () =>
    !document.querySelector(".art") && document.querySelector(".sheet .pill[data-focused]")?.textContent === "Change art",
  );
  await step("5-search", ["Escape", "/"], ".search[data-open] .key[data-focused]");
  await step("6-search-query", ["h", "o", "l"], ".search[data-open] .poster", () =>
    [...document.querySelectorAll(".search .poster")].some((p) => p.getAttribute("aria-label") === "Hollow Knight"),
  );
  await step("7-search-result", ["ArrowDown", "Enter"], ".sheet[data-open]", () =>
    document.querySelector(".sheet-title")?.textContent === "Hollow Knight",
  );
  // Esc walks back: sheet → results → letters → home → menu.
  await step("8-menu", ["Escape", "Escape", "Escape", "Escape"], ".menu[data-open] .menu-item[data-focused]");
  await step("9-home-again", ["Escape"], ".app[data-mode='home'] .card[data-focused]");
  // Like the Steam Deck: Start (Esc) on a focused game, then Change art, and Back returns home.
  await step("9b-menu-change-art", ["Escape", "Enter"], ".art .art-tile[data-focused]", () =>
    document.querySelector(".app")?.getAttribute("data-mode") === "art",
  );
  await step("9c-art-back-home", ["Escape"], ".app[data-mode='home'] .card[data-focused]");
  await step("10-type-to-search", ["c", "e", "l"], ".search[data-open] .poster", () =>
    document.querySelector(".search-query")?.textContent?.startsWith("cel") &&
    [...document.querySelectorAll(".search .poster")].some((p) => p.getAttribute("aria-label") === "Celeste"),
  );
  // Menu → Library layout switches between rows and the grid, and back.
  await step("11-grid-layout", ["Escape", "Escape", "ArrowDown", "ArrowDown", "ArrowDown", "Enter", "Escape"], ".app[data-mode='home'] .row.posters");
  await step("12-rows-layout", ["Escape", "ArrowDown", "ArrowDown", "ArrowDown", "Enter", "Escape"], ".app[data-mode='home'] .track");
  // Menu → Sort rows cycles Recently played → A–Z, and the first row reorders.
  await step("12b-sort", ["Escape", "ArrowDown", "ArrowDown", "ArrowDown", "ArrowDown", "Enter"], ".menu-item[data-focused]", () =>
    document.querySelector(".menu-item[data-focused] .menu-value")?.textContent === "A–Z",
  );
  await step("12c-sorted-row", ["Escape"], ".app[data-mode='home']", () => {
    const names = [...document.querySelectorAll('.shelf[aria-label="Story"] .tile')].map((t) => t.getAttribute("aria-label"));
    return names.length > 2 && names.slice(0, 3).every((n, i, a) => i === 0 || a[i - 1].localeCompare(n, undefined, { sensitivity: "base" }) <= 0);
  });
  // Menu → Desktop mode: the library manager, then back to big screen.
  await step("13-desktop", ["Escape", "ArrowDown", "Enter"], ".desktop .dt-row[data-selected]");
  await page.locator(".dt-row", { hasText: "Hollow Knight" }).click();
  await page.waitForSelector(".dt-inspector input#dt-name");
  if ((await page.inputValue("#dt-name")) !== "Hollow Knight") throw new Error("inspector shows the wrong game");
  await page.fill(".dt-search", "port");
  await page.waitForTimeout(200);
  if ((await page.locator(".dt-body .dt-row").count()) !== 1) throw new Error("desktop search did not filter");
  console.log("ok  desktop list, inspector and search");
  await page.click("text=Big screen");
  await step("14-back-to-big-screen", [], ".app[data-mode='home'] .card[data-focused]");

  if (errors.length) throw new Error(`console errors:\n${errors.join("\n")}`);
} catch (e) {
  failed = true;
  console.error(`FAIL ${e.message}`);
} finally {
  await browser.close();
  server.kill();
}
process.exit(failed ? 1 : 0);
