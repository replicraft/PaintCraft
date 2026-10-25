import assert from 'node:assert/strict';
import test from 'node:test';
import { desktopPlatform, releasesUrl } from '../src/lib/download-platform.js';

test('desktop platforms resolve to the requested release placeholder', () => {
	for (const [platform, expected] of [
		['Win32', 'Windows'],
		['MacIntel', 'macOS'],
		['Linux aarch64', 'Linux'],
		['FreeBSD amd64', 'FreeBSD'],
	]) {
		assert.equal(desktopPlatform({ platform }), expected);
	}
	assert.equal(
		releasesUrl,
		'https://github.com/replicraft/paintcraft/releases/latest',
	);
});
test('mobile and unknown devices keep manual desktop choices', () => {
	for (const navigator of [
		{ userAgent: 'Android Linux' },
		{ platform: 'MacIntel', maxTouchPoints: 5 },
		{ userAgent: 'iPhone' },
		{ userAgentData: { platform: 'Windows', mobile: true } },
		{},
	]) {
		assert.equal(desktopPlatform(navigator), null);
	}
});
