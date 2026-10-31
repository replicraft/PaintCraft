// Main UI produces the same-ABI Export5 packet; fresh Workers replay it exactly.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { capturedExport5, replayExport5 } from './lib/box-export5.mjs';
import { rendered } from './lib/spatial-worker-fixtures.mjs';
import { assetInventory, project, sha } from './lib/studio-filter-oracle.mjs';

const app = fileURLToPath(new URL('../', import.meta.url)),
	root = path.resolve(app, '../..'),
	dist = path.join(app, 'dist');
const kind = process.env.BOX_EXPORT_KIND ?? 'dds',
	phase = process.env.BOX_EXPORT_PHASE ?? 'all',
	c = JSON.parse(process.env.BOX_EXPORT_COORDINATES ?? '{}');
assert.ok(['dds', 'msp'].includes(kind));
assert.ok(
	[
		'layout',
		'formats',
		'settings',
		'options',
		'review',
		'ready',
		'revoked',
		'all',
	].includes(phase),
);
assert.ok(process.env.CODEC_EVIDENCE_DIR && process.env.PLAYWRIGHT_MODULE);
assert.ok(Object.keys(c).length <= 24);
for (const [k, p] of Object.entries(c))
	assert.ok(
		k === 'formatScroll'
			? Number.isFinite(p)
			: Array.isArray(p) && p.length === 2 && p.every(Number.isFinite),
	);
fs.mkdirSync(process.env.CODEC_EVIDENCE_DIR, { recursive: true });
const out = fs.mkdtempSync(
	path.join(process.env.CODEC_EVIDENCE_DIR, 'attempt-'),
);
const folder = path.join(
		root,
		'crates/services/paintcraft-formats/tests/pcnd-spatial-fixtures',
	),
	oracle = JSON.parse(fs.readFileSync(path.join(folder, 'oracle.json'))),
	row = oracle.fixtures.find((r) => r.file === 'box-r2.pcnd');
const source = Buffer.from(fs.readFileSync(path.join(folder, row.file)));
assert.equal(sha(source), row.sha256);
// Independently replace only the eight RAW texel bytes in the frozen tiny file,
// then recompute the outer SHA. No product writer/rendering is an oracle.
const sourcePixels =
	kind === 'dds'
		? [255, 0, 0, 255, 0, 200, 0, 255]
		: [255, 255, 255, 255, 255, 255, 255, 255];
assert.deepEqual(
	[...source.subarray(source.length - 41, source.length - 33)],
	row.raw_rgba,
);
source.set(sourcePixels, source.length - 41);
source.set(
	Buffer.from(sha(source.subarray(0, -32)), 'hex'),
	source.length - 32,
);
const original = project(source),
	expected = rendered(original);
assert.deepEqual(
	expected,
	kind === 'dds' ? [153, 80, 0, 255, 102, 120, 0, 255] : sourcePixels,
);
const inputPath = path.join(out, `box-${kind}.pcnd`);
fs.writeFileSync(inputPath, source);
const assets = assetInventory(dist),
	runtimes = Object.keys(assets).filter((p) =>
		/^_astro\/codec-worker-runtime-.*\.js$/.test(p),
	);
assert.equal(runtimes.length, 1);
const evidence = {
	status: 'started',
	phase,
	kind,
	coordinates: c,
	input: {
		sha256: sha(source),
		parentSha256: row.sha256,
		raw: sourcePixels,
		expected,
	},
	assets,
	checks: [],
	replays: [],
	sources: {},
	startedAt: new Date().toISOString(),
};
for (const file of [
	fileURLToPath(import.meta.url),
	path.join(app, 'scripts/lib/box-export5.mjs'),
	path.join(app, 'scripts/lib/studio-filter-oracle.mjs'),
	path.join(app, 'scripts/lib/studio-box-oracle.mjs'),
	path.join(app, 'scripts/lib/spatial-worker-fixtures.mjs'),
	path.join(folder, 'oracle.json'),
]) {
	evidence.sources[path.relative(root, file)] = sha(fs.readFileSync(file));
	fs.copyFileSync(file, path.join(out, `executed-${path.basename(file)}`));
}
fs.copyFileSync(
	fileURLToPath(import.meta.url),
	path.join(out, 'executed-driver.mjs'),
);
let server, browser, page;
try {
	server = http.createServer((req, res) => {
		const pathname = new URL(req.url, 'http://localhost').pathname,
			file = path.resolve(
				dist,
				`.${pathname === '/' ? '/index.html' : pathname}`,
			);
		if (
			!file.startsWith(`${dist}/`) ||
			!fs.existsSync(file) ||
			!fs.statSync(file).isFile()
		)
			return res.writeHead(404).end();
		res.setHeader(
			'Content-Type',
			{
				'.html': 'text/html',
				'.js': 'text/javascript',
				'.wasm': 'application/wasm',
				'.css': 'text/css',
			}[path.extname(file)] ?? 'application/octet-stream',
		);
		fs.createReadStream(file).pipe(res);
	});
	await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
	const { chromium } = await import(
		pathToFileURL(process.env.PLAYWRIGHT_MODULE).href
	);
	browser = await chromium.launch({
		channel: 'chrome',
		headless: true,
		args: ['--enable-unsafe-swiftshader'],
	});
	evidence.browser = browser.version();
	page = await browser.newPage({
		viewport: { width: 1360, height: 1000 },
		acceptDownloads: true,
	});
	await page.addInitScript(() => {
		localStorage.setItem('showWelcomeScreen', 'false');
		window.__boxPackets = [];
		window.__boxOwners = [];
		window.__boxDownloads = [];
		const Real = window.Worker;
		window.Worker = class extends Real {
			constructor(...a) {
				super(...a);
				this.record = { url: String(a[0]) };
				this.addEventListener('message', ({ data }) => {
					if (
						data.bytes instanceof ArrayBuffer &&
						data.bytes.byteLength <= 1048576
					) {
						this.record.response = [...new Uint8Array(data.bytes)];
						this.record.responseKind = data.kind;
					}
				});
			}
			postMessage(value, transfer) {
				if (value.kind === 'open' && value.plan?.owner)
					window.__boxOwners.push(structuredClone(value.plan));
				if (
					value.bytes instanceof ArrayBuffer &&
					value.bytes.byteLength <= 1048576 &&
					['dds-spatial-export', 'msp-spatial-export'].includes(
						value.kind,
					)
				) {
					this.record.kind = value.kind;
					this.record.request = [...new Uint8Array(value.bytes)];
					window.__boxPackets.push(this.record);
				}
				return super.postMessage(value, transfer);
			}
		};
	});
	page.on('download', (d) =>
		evidence.checks.push({
			event: 'download',
			name: d.suggestedFilename(),
		}),
	);
	page.on('pageerror', (e) => {
		evidence.pageErrors ??= [];
		evidence.pageErrors.push(String(e));
	});
	await page.goto(`http://127.0.0.1:${server.address().port}/`);
	await page.waitForFunction(
		() => window.paintcraft && document.querySelector('#loading').hidden,
		null,
		{ timeout: 60000 },
	);
	const settle = () => page.waitForTimeout(400),
		click = async (p) => {
			await page.mouse.click(...p);
			await settle();
		},
		at = async (k) => {
			assert.ok(c[k], `Calibrate ${k}`);
			await click(c[k]);
		},
		shot = (label) =>
			page.screenshot({ path: path.join(out, `${label}.png`) });
	await click([1273, 20]);
	await click([1160, 98]);
	const chooser = page.waitForEvent('filechooser');
	chooser.catch(() => {});
	await click([20, 46]);
	await click([80, 94]);
	await (await chooser).setFiles(inputPath);
	await settle();
	const save = async (name, trigger) => {
		const pending = page.waitForEvent('download');
		pending.catch(() => {});
		await trigger();
		const d = await pending;
		const p = path.join(out, name);
		await d.saveAs(p);
		await settle();
		return fs.readFileSync(p);
	};
	const before = await save('before.pcnd', () =>
		page.keyboard.press('Meta+s'),
	);
	assert.deepEqual(project(before), original);
	await page.waitForFunction(
		(state) => window.__boxOwners.some((p) => p.owner.state === state),
		original.state,
		{ timeout: 30000 },
	);
	const expectedOwner = await page.evaluate(
		(state) =>
			window.__boxOwners.find((p) => p.owner.state === state).owner,
		original.state,
	);
	evidence.expectedSessionOwner = expectedOwner;
	const checkOwner = (receipt) => {
		assert.equal(receipt.owner, expectedOwner.session);
		assert.equal(receipt.epoch, expectedOwner.epoch);
		assert.equal(receipt.state, expectedOwner.state);
	};
	const title = await page.title(),
		dirty = await page.evaluate(() => window.paintcraft.is_dirty());
	await shot('layout');
	if (phase === 'layout') evidence.status = 'calibration-only';
	else {
		await click([20, 46]);
		await click([80, 242]);
		await click(c.formatBox ?? [540, 414]);
		await page.mouse.move(540, 455);
		await page.mouse.wheel(0, -2000);
		await settle();
		await page.mouse.wheel(0, c.formatScroll ?? 900);
		await settle();
		await shot('formats');
		if (phase === 'formats') evidence.status = 'calibration-only';
		else {
			await at('formatChoice');
			await shot('settings');
			if (phase === 'settings') evidence.status = 'calibration-only';
			else {
				// DDS mip2 forces advanced Worker; MSP must explicitly select RLE2.
				await at('settingBox');
				await shot('setting-options');
				if (phase === 'options') evidence.status = 'calibration-only';
				else {
					await at('settingChoice');
					await at('prepare');
					await shot('unchecked-review');
					assert.equal(
						(await page.evaluate(() => window.__boxPackets)).length,
						0,
						'default BakeRefuse must not launch Worker',
					);
					if (phase === 'review')
						evidence.status = 'calibration-only';
					else {
						await at('bake');
						await page.waitForFunction(
							(kind) =>
								window.__boxPackets.some(
									(p) =>
										p.kind === `${kind}-spatial-export` &&
										p.responseKind === 'result',
								),
							kind,
							{ timeout: 30000 },
						);
						await settle();
						await shot('ready');
						if (phase === 'ready')
							evidence.status = 'calibration-only';
						else {
							const first = (
								await page.evaluate(() => window.__boxPackets)
							).at(-1);
							assert.ok(first);
							evidence.first = capturedExport5(
								first.request,
								kind,
								original,
								row.stack_sha256,
								[0, 0, 0, 2, 0, 0, 0, 0],
							);
							checkOwner(evidence.first);
							await at('readyBake');
							await shot('revoked');
							if (phase === 'revoked') {
								evidence.status = 'calibration-only';
							} else {
								let forbidden = false;
								const watch = () => {
									forbidden = true;
								};
								page.on('download', watch);
								await at('revokedExport');
								await settle();
								page.off('download', watch);
								assert.equal(
									forbidden,
									false,
									'revoked witness cannot download',
								);
								const packetCount = await page.evaluate(
									() => window.__boxPackets.length,
								);
								assert.equal(packetCount, 1);
								await at('bake');
								await page.waitForFunction(
									(kind) =>
										window.__boxPackets.filter(
											(p) =>
												p.kind ===
													`${kind}-spatial-export` &&
												p.responseKind === 'result',
										).length === 2,
									kind,
									{ timeout: 30000 },
								);
								await settle();
								const records = await page.evaluate(
										() => window.__boxPackets,
									),
									last = records.at(-1);
								assert.equal(records.length, 2);
								evidence.final = capturedExport5(
									last.request,
									kind,
									original,
									row.stack_sha256,
									[0, 0, 0, 2, 0, 0, 0, 0],
								);
								checkOwner(evidence.final);
								assert.notEqual(
									evidence.first.nonce,
									evidence.final.nonce,
									'recheck creates fresh nonce',
								);
								const output = await save(`box.${kind}`, () =>
									at('export'),
								);
								await settle();
								const after = await save('after.pcnd', () =>
									page.keyboard.press('Meta+s'),
								);
								assert.deepEqual(after, before);
								assert.equal(await page.title(), title);
								assert.equal(
									await page.evaluate(() =>
										window.paintcraft.is_dirty(),
									),
									dirty,
								);
								// Replay the actual same-ABI producer packet into fresh isolated Workers.
								for (const mutation of [
									null,
									'bake',
									'source',
									'owner',
									'stored',
									'workspace',
									'reserved',
									'old-version',
									'future-version',
								]) {
									const result = await page.evaluate(
										replayExport5,
										{
											request: last.request,
											kind,
											runtime: `/${runtimes[0]}`,
											expected,
											mutation,
										},
									);
									evidence.replays.push(result);
									if (!mutation)
										assert.deepEqual(
											Buffer.from(result.encoded),
											output,
											'download equals fresh replay artifact',
										);
								}
								fs.writeFileSync(
									path.join(out, 'captured-request.bin'),
									Buffer.from(last.request),
								);
								fs.writeFileSync(
									path.join(out, 'captured-response.bin'),
									Buffer.from(last.response),
								);
								const python = process.env.PILLOW_PYTHON;
								assert.ok(
									python,
									'Existing independent Pillow interpreter required',
								);
								const version = spawnSync(
									python,
									[
										'-B',
										'-c',
										'import PIL,sys; print(sys.version); print(PIL.__version__)',
									],
									{ encoding: 'utf8' },
								);
								assert.equal(version.status, 0, version.stderr);
								evidence.independentDecoder = {
									interpreter: python,
									version: version.stdout.trim(),
								};
								const decode = spawnSync(
									python,
									[
										'-B',
										'-c',
										'from PIL import Image; import sys; p=Image.open(sys.argv[1]); assert p.size==(2,1); sys.stdout.buffer.write(p.convert("RGBA").tobytes())',
										path.join(out, `box.${kind}`),
									],
									{
										env: {
											...process.env,
											PYTHONDONTWRITEBYTECODE: '1',
										},
									},
								);
								assert.equal(
									decode.status,
									0,
									decode.stderr.toString(),
								);
								assert.deepEqual([...decode.stdout], expected);
								evidence.independentPixels = [...decode.stdout];
								evidence.checks.push(
									'checked/revoked/rechecked exact producer packet, before/after byte-identical project/title/dirty, fresh Worker replay and eight tamper controls, independent Pillow2x1',
								);
								assert.deepEqual(assetInventory(dist), assets);
								evidence.status = 'passed';
							}
						}
					}
				}
			}
		}
	}
} catch (error) {
	evidence.status = 'failed';
	evidence.error = String(error.stack ?? error);
	process.exitCode = 1;
} finally {
	try {
		await browser?.close();
	} finally {
		try {
			if (server?.listening)
				await new Promise((resolve) => server.close(resolve));
		} finally {
			evidence.finishedAt = new Date().toISOString();
			fs.writeFileSync(
				path.join(out, 'final.json'),
				JSON.stringify(evidence, null, 2),
			);
			console.log(
				JSON.stringify({ out, status: evidence.status, phase, kind }),
			);
		}
	}
}
