import test from 'node:test';
import assert from 'node:assert/strict';
import { fullscreenActive, toggleFullscreen } from '../src/lib/fullscreen.js';

function browser() {
	const doc = {
		documentElement: {},
		fullscreenElement: null,
		fullscreenEnabled: true,
	};
	doc.documentElement.requestFullscreen = async () => {
		doc.fullscreenElement = doc.documentElement;
	};
	doc.exitFullscreen = async () => {
		doc.fullscreenElement = null;
	};
	return doc;
}

test('entry and exit follow completed browser requests and external exits', async () => {
	const doc = browser();
	assert.equal(fullscreenActive(doc), false);
	assert.equal(await toggleFullscreen(doc), true);
	assert.equal(await toggleFullscreen(doc), false);
	await toggleFullscreen(doc);
	doc.fullscreenElement = null; // Browser Escape or window-management exit.
	assert.equal(fullscreenActive(doc), false);
});

test('pending entry never claims fullscreen before the browser changes it', async () => {
	const doc = browser();
	let complete;
	doc.documentElement.requestFullscreen = () =>
		new Promise((resolve) => {
			complete = () => {
				doc.fullscreenElement = doc.documentElement;
				resolve();
			};
		});
	const request = toggleFullscreen(doc);
	assert.equal(fullscreenActive(doc), false);
	complete();
	assert.equal(await request, true);
});

test('denied entry and exit are catchable and retain actual browser state', async () => {
	const doc = browser();
	doc.documentElement.requestFullscreen = () =>
		Promise.reject(new Error('Permission denied'));
	await assert.rejects(toggleFullscreen(doc), /Permission denied/);
	assert.equal(fullscreenActive(doc), false);
	doc.fullscreenElement = doc.documentElement;
	doc.exitFullscreen = () => Promise.reject(new Error('Exit denied'));
	await assert.rejects(toggleFullscreen(doc), /Exit denied/);
	assert.equal(fullscreenActive(doc), true);
});

test('missing browser support is an explicit failure without changing the document', async () => {
	const doc = browser();
	doc.fullscreenEnabled = false;
	await assert.rejects(toggleFullscreen(doc), /Fullscreen unavailable/);
	assert.equal(fullscreenActive(doc), false);
	assert.equal(fullscreenActive({}), false);
	await assert.rejects(toggleFullscreen({}), /No browser document/);
});
