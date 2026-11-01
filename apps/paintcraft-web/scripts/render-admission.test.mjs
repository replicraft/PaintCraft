import assert from 'node:assert/strict';
import test from 'node:test';
import {
	createRenderAdmission,
	renderAdmission,
} from '../src/lib/render-admission.js';
import {
	beginFilterPreview,
	filterPreviewUsage,
} from '../src/lib/filter-preview-worker.js';
const MiB = 1024 * 1024;
const rejected = (f) => assert.throws(f, { code: 'ResourceLimit' });
test('legacy and appearance leases share one exclusive owner', () => {
	const broker = createRenderAdmission();
	const first = broker.acquire(448 * MiB);
	rejected(() => broker.acquire(448 * MiB));
	first.release();
	const next = broker.acquire(448 * MiB);
	first.release(); // A late old ACK cannot release the new owner.
	assert.equal(broker.usage().reserved, 448 * MiB);
	next.release();
	assert.deepEqual(broker.usage(), { reserved: 0, quarantined: false });
});
test('close ACK retains the envelope until final Main storage guard drops', () => {
	const broker = createRenderAdmission();
	const lease = broker.acquire(448 * MiB);
	const main = lease.reserveMain(64 * MiB);
	assert.equal(broker.usage().reserved, 448 * MiB);
	lease.release();
	rejected(() => broker.acquire(1));
	main.release();
	main.release();
	assert.equal(broker.usage().reserved, 0);
});
test('Main credit may drop before ACK without releasing a live Worker', () => {
	const broker = createRenderAdmission();
	const lease = broker.acquire(448 * MiB);
	lease.reserveMain(1).release();
	assert.equal(broker.usage().reserved, 448 * MiB);
	lease.release();
	assert.equal(broker.usage().reserved, 0);
});
test('invalid preparation requests do not consume credit or mint extra allowance', () => {
	const broker = createRenderAdmission();
	const lease = broker.acquire(448 * MiB);
	for (const n of [0, -1, 1.5, NaN, Infinity, '1', 64 * MiB + 1])
		rejected(() => lease.reserveMain(n));
	const main = lease.reserveMain(64 * MiB);
	rejected(() => lease.reserveMain(1));
	main.release();
	rejected(() => lease.reserveMain(1)); // A new context needs a new OPEN.
	lease.release();
});
test('quarantine persists after ACK and Main release', () => {
	const broker = createRenderAdmission();
	const lease = broker.acquire(448 * MiB);
	const main = lease.reserveMain(1);
	lease.quarantine();
	lease.release();
	main.release();
	assert.deepEqual(broker.usage(), {
		reserved: 448 * MiB,
		quarantined: true,
	});
	rejected(() => broker.acquire(1));
	rejected(() => lease.reserveMain(1));
	rejected(() => lease.resize(449 * MiB));
});
test('retired handles cannot grant preparation storage', () => {
	const broker = createRenderAdmission();
	const lease = broker.acquire(448 * MiB);
	lease.release();
	rejected(() => lease.reserveMain(1));
});
test('only actual control growth before Main preparation can resize declaration', () => {
	const broker = createRenderAdmission();
	const lease = broker.acquire(448 * MiB);
	lease.resize(448 * MiB + 4096);
	assert.equal(broker.usage().reserved, 448 * MiB + 4096);
	rejected(() => lease.resize(448 * MiB));
	rejected(() => lease.resize('900000000'));
	lease.reserveMain(1).release();
	rejected(() => lease.resize(449 * MiB));
	lease.release();
});

test('normal legacy exports share the exact production appearance admission owner', () => {
	const id = '11111111-1111-1111-1111-111111111111';
	const plan = {
		owner: { session: id, state: id, epoch: '0', frontend: 3 },
		draft: id,
		document: id,
		size: [3, 2],
		layers: 1,
		residentBytes: 262144,
		metadataBytes: 0,
		metadataCharge: 0,
		maskCount: 0,
		maskBands: 0,
		maskRuns: 0,
		maskDecodeBytes: 0,
		filterDigest: '0'.repeat(64),
		sourceSha256: '1'.repeat(64),
	};
	const legacy = beginFilterPreview(plan);
	assert.deepEqual(filterPreviewUsage(), renderAdmission.usage());
	rejected(() => renderAdmission.acquire(448 * MiB));
	legacy.cancel();
	const appearance = renderAdmission.acquire(448 * MiB);
	const main = appearance.reserveMain(64 * MiB);
	rejected(() => beginFilterPreview(plan));
	appearance.release();
	rejected(() => beginFilterPreview(plan));
	main.release();
	assert.equal(filterPreviewUsage().reserved, 0);
	beginFilterPreview(plan).cancel();
});
