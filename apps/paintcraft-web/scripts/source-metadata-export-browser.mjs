// Actual production UI: explicit metadata omission without mutating the project. No Rust test hooks.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import {
	pngSamples,
	projectAttachment,
} from './lib/metadata-export-oracles.mjs';

const app = fileURLToPath(new URL('../', import.meta.url));
const root = path.resolve(app, '../..');
const dist = path.join(app, 'dist');
const base = process.env.CODEC_EVIDENCE_DIR;
if (!base || !process.env.PLAYWRIGHT_MODULE)
	throw Error('Set evidence directory and existing Playwright module');
fs.mkdirSync(base, { recursive: true });
const out = fs.mkdtempSync(path.join(base, 'attempt-'));
const phase = process.env.METADATA_BROWSER_PHASE ?? 'finish';
const evidence = {
	status: 'started',
	phase,
	sources: {},
	assets: {},
	checks: [],
	startedAt: new Date().toISOString(),
};
const sha = (bytes) => createHash('sha256').update(bytes).digest('hex');
function inventory(folder) {
	for (const entry of fs.readdirSync(folder, { withFileTypes: true })) {
		const file = path.join(folder, entry.name);
		if (entry.isDirectory()) inventory(file);
		else
			evidence.assets[path.relative(dist, file)] = {
				bytes: fs.statSync(file).size,
				sha256: sha(fs.readFileSync(file)),
			};
	}
}
inventory(dist);
for (const name of [
	'Cargo.lock',
	'apps/paintcraft-web/src/paintcraft-web/app.rs',
	'apps/paintcraft-web/src/paintcraft-web/app/exporting.rs',
	'apps/paintcraft-web/src/paintcraft-web/app/exporting/frozen.rs',
	'apps/paintcraft-web/src/paintcraft-web/files.rs',
	'apps/paintcraft-web/src/paintcraft-web/files/flat_export.rs',
	'apps/paintcraft-web/src/paintcraft-web/text_ui.rs',
	'crates/services/paintcraft-formats/src/export.rs',
	'crates/services/paintcraft-formats/src/export/metadata_policy.rs',
	'crates/presentation/paintcraft-ui/src/source_metadata_export.rs',
	'apps/paintcraft-web/scripts/source-metadata-export-browser.mjs',
])
	evidence.sources[name] = sha(fs.readFileSync(path.join(root, name)));
const fixtures = path.join(
	root,
	'crates/services/paintcraft-formats/tests/png-retention-fixtures',
);
const source = path.join(fixtures, 'rgb8.png');
const expected = fs.readFileSync(`${source}.rgba`);
const original = fs.readFileSync(source);
const profile = fs.readFileSync(
	path.join(root, 'assets/profiles/sRGB2014.icc'),
);
const oracle = JSON.parse(
	fs.readFileSync(path.join(fixtures, 'oracle.json')),
).fixtures.find((row) => row.file === 'rgb8.png');
assert.equal(sha(original), oracle.sha256);
assert.equal(sha(expected), oracle.rgba_sha256);
assert.equal(sha(profile), oracle.profile_sha256);
evidence.input = { path: path.relative(root, source), ...oracle };
for (const name of [
	'apps/paintcraft-web/scripts/lib/metadata-export-oracles.mjs',
	'crates/services/paintcraft-formats/tests/png-retention-fixtures/rgb8.png',
	'crates/services/paintcraft-formats/tests/png-retention-fixtures/rgb8.png.rgba',
	'crates/services/paintcraft-formats/tests/png-retention-fixtures/oracle.json',
	'assets/profiles/sRGB2014.icc',
])
	evidence.sources[name] = sha(fs.readFileSync(path.join(root, name)));
const { chromium } = await import(
	pathToFileURL(process.env.PLAYWRIGHT_MODULE).href
);
let browser, server, page;
try {
	server = http.createServer((request, response) => {
		const pathname = new URL(request.url, 'http://localhost').pathname;
		const file = path.resolve(
			dist,
			`.${pathname.endsWith('/') ? `${pathname}index.html` : pathname}`,
		);
		if (!file.startsWith(`${dist}/`) || !fs.existsSync(file)) {
			response.writeHead(404);
			response.end();
			return;
		}
		response.setHeader(
			'Content-Type',
			{
				'.html': 'text/html',
				'.js': 'text/javascript',
				'.wasm': 'application/wasm',
				'.css': 'text/css',
				'.json': 'application/json',
			}[path.extname(file)] ?? 'application/octet-stream',
		);
		fs.createReadStream(file).pipe(response);
	});
	await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
	browser = await chromium.launch({
		channel: 'chrome',
		headless: true,
		args: ['--enable-unsafe-swiftshader'],
	});
	evidence.browser = browser.version();
	const c = JSON.parse(process.env.METADATA_BROWSER_COORDINATES ?? '{}');
	async function freshPage() {
		page = await browser.newPage({
			viewport: { width: 1360, height: 1000 },
			acceptDownloads: true,
		});
		await page.addInitScript(() =>
			localStorage.setItem('showWelcomeScreen', 'false'),
		);
		page.on('pageerror', (error) => {
			evidence.pageErrors ??= [];
			evidence.pageErrors.push(String(error));
		});
		await page.goto(`http://127.0.0.1:${server.address().port}/`, {
			waitUntil: 'networkidle',
		});
		await page.waitForFunction(
			() =>
				window.paintcraft && document.querySelector('#loading').hidden,
			null,
			{ timeout: 60000 },
		);
		await click(1273, 20);
		await click(1160, 98);
	}
	const click = async (x, y) => {
		await page.mouse.click(x, y);
		await page.waitForTimeout(400);
	};
	async function openFile(file) {
		const chooser = page.waitForEvent('filechooser');
		await click(20, 46);
		await click(80, 94);
		await (await chooser).setFiles(file);
		await page.waitForTimeout(700);
	}
	await freshPage();
	const oldTitle = await page.title();
	await openFile(source);
	await page.screenshot({ path: path.join(out, 'import-unchecked.png') });
	if (phase === 'import') {
		evidence.status = 'calibration-only';
	} else {
		assert.ok(
			c.importCheckbox && c.importOpen,
			'Set observed import-checkbox/Open coordinates',
		);
		await click(...c.importOpen);
		assert.equal(
			await page.title(),
			oldTitle,
			'unchecked import does not adopt',
		);
		assert.equal(
			await page.evaluate(() => window.paintcraft.is_dirty()),
			false,
		);
		await click(...c.importCheckbox);
		await page.screenshot({ path: path.join(out, 'import-checked.png') });
		await click(...c.importOpen);
		await page.waitForFunction(() => window.paintcraft.is_dirty());
		await page.screenshot({ path: path.join(out, 'imported.png') });
		async function download(name, trigger) {
			const pending = page.waitForEvent('download');
			await trigger();
			const item = await pending;
			await item.saveAs(path.join(out, name));
			assert.equal(await item.failure(), null);
			evidence.checks.push({
				download: name,
				sha256: sha(fs.readFileSync(path.join(out, name))),
			});
		}
		await download('imported.pcnd', () => page.keyboard.press('Meta+s'));
		assert.equal(
			await page.evaluate(() => window.paintcraft.is_dirty()),
			true,
			'Download never claims a confirmed disk save',
		);
		evidence.importAttachment = projectAttachment(
			fs.readFileSync(path.join(out, 'imported.pcnd')),
			original,
			profile,
			oracle.width,
			oracle.height,
		);
		await page.close();
		await freshPage();
		await openFile(path.join(out, 'imported.pcnd'));
		await page.waitForFunction(
			() => document.title === 'imported.pcnd - PaintCraft',
		);
		const title = await page.title();
		assert.equal(
			await page.evaluate(() => window.paintcraft.is_dirty()),
			false,
		);
		await download('before.pcnd', () => page.keyboard.press('Meta+s'));
		assert.deepEqual(
			fs.readFileSync(path.join(out, 'imported.pcnd')),
			fs.readFileSync(path.join(out, 'before.pcnd')),
		);
		evidence.reopenedAttachment = projectAttachment(
			fs.readFileSync(path.join(out, 'before.pcnd')),
			original,
			profile,
			oracle.width,
			oracle.height,
		);

		async function openExport() {
			await click(20, 46);
			await click(80, 242);
		}
		await openExport();
		await page.screenshot({ path: path.join(out, 'unchecked.png') });
		if (phase === 'options') evidence.status = 'calibration-only';
		else {
			assert.ok(
				c.checkbox && c.download && c.cancel,
				'Set observed checkbox/download/cancel coordinates',
			);
			let unexpected = 0;
			const observe = () => unexpected++;
			page.on('download', observe);
			await click(...c.download);
			await page.waitForTimeout(1200);
			assert.equal(
				unexpected,
				0,
				'unchecked metadata choice must block actual download',
			);
			page.off('download', observe);
			await click(...c.cancel);
			await download('cancel.pcnd', () => page.keyboard.press('Meta+s'));
			assert.deepEqual(
				fs.readFileSync(path.join(out, 'before.pcnd')),
				fs.readFileSync(path.join(out, 'cancel.pcnd')),
			);
			await openExport();
			await click(...c.checkbox);
			await page.screenshot({ path: path.join(out, 'checked.png') });
			await download('explicit.png', () => click(...c.download));
			evidence.copyPixels = pngSamples(
				fs.readFileSync(path.join(out, 'explicit.png')),
				oracle.width,
				oracle.height,
				expected,
			);
			assert.equal(await page.title(), title);
			assert.equal(
				await page.evaluate(() => window.paintcraft.is_dirty()),
				false,
			);
			await download('after.pcnd', () => page.keyboard.press('Meta+s'));
			assert.deepEqual(
				fs.readFileSync(path.join(out, 'before.pcnd')),
				fs.readFileSync(path.join(out, 'after.pcnd')),
			);
			assert.equal(evidence.pageErrors?.length ?? 0, 0);
			evidence.status = 'passed';
			evidence.checks.push(
				'Real PNG visible consent adopts unsaved exact-ICC/resolution metadata; PCND2 save/reopen retains exact attachment; unchecked export blocks; Cancel and explicit numeric PNG copy preserve PCND/title/dirty',
			);
		}
	}
} catch (error) {
	evidence.status = 'failed';
	evidence.failure = String(error.stack ?? error);
	await page
		?.screenshot({ path: path.join(out, 'failure.png') })
		.catch(() => {});
	throw error;
} finally {
	try {
		for (const [name, hash] of Object.entries(evidence.sources))
			assert.equal(
				sha(fs.readFileSync(path.join(root, name))),
				hash,
				`source changed: ${name}`,
			);
		for (const [name, value] of Object.entries(evidence.assets))
			assert.equal(
				sha(fs.readFileSync(path.join(dist, name))),
				value.sha256,
				`asset changed: ${name}`,
			);
	} catch (error) {
		evidence.status = 'failed';
		evidence.postflightFailure = String(error.stack ?? error);
		process.exitCode = 1;
	}
	evidence.finishedAt = new Date().toISOString();
	fs.writeFileSync(
		path.join(out, 'production.json'),
		`${JSON.stringify(evidence, null, 4)}\n`,
	);
	console.log(JSON.stringify({ out, status: evidence.status, phase }));
	await browser?.close();
	await new Promise((resolve) => server?.close(resolve) ?? resolve());
}
