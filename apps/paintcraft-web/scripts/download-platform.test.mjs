import assert from 'node:assert/strict';
import test from 'node:test';
import {
	assetName,
	assetUrl,
	desktopPlatform,
	downloadTarget,
	platforms,
	releaseAssets,
} from '../src/lib/download-platform.js';

test('desktop downloads select all six published OS and CPU combinations', async () => {
	for (const [navigator, id, suffix] of [
		[
			{ platform: 'Win32', userAgent: 'Windows NT 10.0; Win64; x64' },
			'windows-x64',
			'.msi',
		],
		[
			{ platform: 'Win32', userAgent: 'Windows NT 10.0; ARM64' },
			'windows-arm64',
			'.msi',
		],
		[{ platform: 'MacIntel' }, 'macos-x64', '.dmg'],
		[
			{
				platform: 'MacIntel',
				userAgentData: {
					platform: 'macOS',
					getHighEntropyValues: async () => ({
						architecture: 'arm',
						bitness: '64',
					}),
				},
			},
			'macos-arm64',
			'.dmg',
		],
		[{ platform: 'Linux x86_64' }, 'linux-x64', '.AppImage'],
		[{ platform: 'Linux aarch64' }, 'linux-arm64', '.AppImage'],
	]) {
		const target = await downloadTarget(navigator);
		assert.equal(target.id, id);
		assert.equal(target.suffix, suffix);
	}
});

test('client hints detect Windows ARM even when the user agent says x64', async () => {
	const target = await downloadTarget({
		platform: 'Win32',
		userAgent: 'Windows NT 10.0; Win64; x64',
		userAgentData: {
			platform: 'Windows',
			getHighEntropyValues: async () => ({
				architecture: 'arm',
				bitness: '64',
			}),
		},
	});
	assert.equal(target.id, 'windows-arm64');
});

test('Mac renderer fallback selects Apple silicon and denied hints remain usable', async () => {
	assert.equal(
		(await downloadTarget({ platform: 'MacIntel' }, () => 'arm64')).id,
		'macos-arm64',
	);
	assert.equal(
		(
			await downloadTarget(
				{
					platform: 'MacIntel',
					userAgentData: {
						getHighEntropyValues: async () => {
							throw new Error('Denied');
						},
					},
				},
				() => 'x64',
			)
		).id,
		'macos-x64',
	);
});

test('mobile, unsupported 32-bit Linux, BSD and unknown devices do not auto-download', async () => {
	for (const navigator of [
		{ userAgent: 'Android Linux' },
		{ platform: 'MacIntel', maxTouchPoints: 5 },
		{ userAgent: 'iPhone' },
		{ userAgentData: { platform: 'Windows', mobile: true } },
		{ platform: 'FreeBSD amd64', userAgent: 'X11' },
		{},
	]) {
		assert.equal(desktopPlatform(navigator), null);
		assert.equal(await downloadTarget(navigator), null);
	}
	assert.equal(await downloadTarget({ platform: 'Linux i686' }), null);
});

test('the catalog contains all 24 distinct published desktop and CLI assets', () => {
	const names = platforms.flatMap((platform) =>
		platform.architectures.flatMap((architecture) => {
			const target = `${platform.id}-${architecture.id}`;
			return [
				assetName('0.1.0', target, platform.cli, true),
				...platform.formats.map((format) =>
					assetName('0.1.0', target, format.suffix),
				),
			];
		}),
	);
	assert.equal(names.length, 24);
	assert.equal(new Set(names).size, 24);
	assert.ok(names.includes('paintcraft-cli-0.1.0-windows-arm64.zip'));
	assert.ok(names.includes('paintcraft-0.1.0-macos-arm64-update.zip'));
	assert.ok(names.includes('paintcraft-0.1.0-linux-x64.AppImage'));
});

test('latest release metadata resolves direct assets and rejects unavailable releases', () => {
	const name = 'paintcraft-0.2.0-windows-x64.msi';
	const latest = releaseAssets({
		tag_name: 'v0.2.0',
		assets: [
			{ name, size: 123, state: 'uploaded' },
			{ name: 'incomplete', size: 0, state: 'uploading' },
		],
	});
	assert.equal(latest.version, '0.2.0');
	assert.equal(
		latest.assets.get(name),
		'https://github.com/replicraft/paintcraft/releases/download/v0.2.0/paintcraft-0.2.0-windows-x64.msi',
	);
	assert.equal(latest.assets.size, 1);
	assert.equal(
		assetUrl('v0.1.0', 'SHA256SUMS.txt'),
		'https://github.com/replicraft/paintcraft/releases/download/v0.1.0/SHA256SUMS.txt',
	);
	for (const release of [
		{},
		{ tag_name: '../../elsewhere', assets: [] },
		{ tag_name: 'v0.2.0', assets: [] },
	])
		assert.throws(() => releaseAssets(release));
});
