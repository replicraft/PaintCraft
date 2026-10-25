// Independent Chromium rendering. Supply an isolated Playwright package location.
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";

const require = createRequire(
	process.env.PLAYWRIGHT_PACKAGE_JSON || import.meta.url,
);
const { chromium } = require("playwright");
const root = path.dirname(fileURLToPath(import.meta.url));
const font = await fs.readFile(
	path.join(root, "../../../../../assets/fonts/selawik/selawk.ttf"),
);
const manifest = JSON.parse(
	await fs.readFile(path.join(root, "sources.json"), "utf8"),
);
const browser = await chromium.launch({ channel: "chrome", headless: true });
try {
	const page = await browser.newPage({
		viewport: { width: 256, height: 128 },
		deviceScaleFactor: 1,
	});
	const errors = [];
	page.on("pageerror", (error) => errors.push(String(error)));
	await page.route("**/*", (route) => route.abort());
	for (const fixture of manifest.files) {
		const source = await fs.readFile(
			path.join(root, `${fixture.name}.svg`),
		);
		assert.equal(
			createHash("sha256").update(source).digest("hex"),
			fixture.source_sha256,
		);
		await page.setContent(
			`<style>html,body { margin:0; padding:0; background:transparent; }</style>${source}`,
		);
		await page.evaluate(async (base64) => {
			const data = Uint8Array.from(atob(base64), (ch) =>
				ch.charCodeAt(0),
			);
			const fontFace = await new FontFace("Selawik", data).load();
			document.fonts.add(fontFace);
			await document.fonts.ready;
		}, font.toString("base64"));
		const box = await page.locator("svg").boundingBox();
		assert.equal(box.width, fixture.width);
		assert.equal(box.height, fixture.height);
		fixture.image_rendering = await page
			.locator("image")
			.evaluateAll((nodes) =>
				nodes.map((node) => getComputedStyle(node).imageRendering),
			);
		const png = await page
			.locator("svg")
			.screenshot({ omitBackground: true, animations: "disabled" });
		await fs.writeFile(path.join(root, `${fixture.name}.chrome.png`), png);
		fixture.png_sha256 = createHash("sha256").update(png).digest("hex");
	}
	assert.deepEqual(errors, []);
	manifest.oracle = `Google Chrome ${browser.version()}, Playwright ${require("playwright/package.json").version}`;
	manifest.font_sha256 = createHash("sha256").update(font).digest("hex");
	manifest.css_smooth_supported = await page.evaluate(() =>
		CSS.supports("image-rendering", "smooth"),
	);
	await fs.writeFile(
		path.join(root, "oracle.json"),
		`${JSON.stringify(manifest, null, 4)}\n`,
	);
	console.log(
		`Verified and rendered ${manifest.files.length} fixtures with ${manifest.oracle}`,
	);
} finally {
	await browser.close();
}
