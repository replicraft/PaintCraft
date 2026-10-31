import assert from 'node:assert/strict';
import test from 'node:test';
import {
	SPATIAL_JOB_WORK_MAX,
	SPATIAL_WORKSPACE_BYTES_MAX,
	validateBinding,
	validatePlan,
	validateRequest,
} from '../src/lib/filter-preview-protocol.js';

const uuid = '00000000-0000-4000-8000-000000000001';
const legacy = {
	owner: { session: uuid, state: uuid, epoch: '0', frontend: 3 },
	draft: uuid,
	document: uuid,
	size: [256, 256],
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
const spatial = {
	...legacy,
	protocol: 3,
	spatialCount: 0,
	workspaceBytes: SPATIAL_WORKSPACE_BYTES_MAX,
	workLimit: SPATIAL_JOB_WORK_MAX,
};
const request = {
	generation: '1',
	operation: {
		op: 'filter.add',
		descriptor: { kind: 'box-blur-v1', radius: 0 },
	},
	target: uuid,
	view: { region: [0, 0, 256, 256], level: 8 },
	render: true,
};
test('legacy source shape stays closed and cannot authorize spatial private controls', () => {
	validatePlan(legacy);
	for (const [key, value] of Object.entries({
		protocol: 2,
		spatialCount: 0,
		workspaceBytes: 0,
		workLimit: 0,
	}))
		assert.throws(() => validatePlan({ ...legacy, [key]: value }));
	assert.throws(
		() => validateRequest(request, legacy),
		(e) => e.code === 'CapabilityUnsupported',
	);
	validateRequest(
		{
			...request,
			operation: {
				op: 'filter.add',
				descriptor: { kind: 'invert-rgb-v1' },
			},
		},
		legacy,
	);
});
test('spatial profile admits zero stored source for AddBox but binds every workspace fact in READY', () => {
	validatePlan(spatial);
	validateRequest(request, spatial);
	validateBinding(spatial, spatial);
	for (const [key, value] of Object.entries({
		spatialCount: 1,
		workspaceBytes: 1,
		workLimit: 1,
		residentBytes: 0,
		metadataBytes: 1,
		metadataCharge: 1,
		layers: 2,
	}))
		assert.throws(() =>
			validateBinding({ ...spatial, [key]: value }, spatial),
		);
	assert.throws(() => validateBinding(spatial, legacy));
	assert.throws(() => validateBinding(legacy, spatial));
});
test('spatial source rejects unknown profiles, partial facts and excessive bounded declarations', () => {
	for (const protocol of [1, 2, 4, '3'])
		assert.throws(() => validatePlan({ ...spatial, protocol }));
	for (const key of ['spatialCount', 'workspaceBytes', 'workLimit']) {
		const missing = { ...spatial };
		delete missing[key];
		assert.throws(() => validatePlan(missing));
		assert.throws(() => validatePlan({ ...spatial, [key]: -1 }));
	}
	for (const [key, value] of Object.entries({
		spatialCount: 257,
		workspaceBytes: SPATIAL_WORKSPACE_BYTES_MAX + 1,
		workLimit: SPATIAL_JOB_WORK_MAX + 1,
	}))
		assert.throws(
			() => validatePlan({ ...spatial, [key]: value }),
			(e) => e.code === 'ResourceLimit',
		);
});
