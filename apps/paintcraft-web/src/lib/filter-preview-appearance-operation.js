// Closed Source7 numeric operation grammar; masks publish through Editor/reopen.
import { encodeProgram } from './filter-preview-program-protocol.js';
import {
	fail7,
	numericalEntry7,
	shape7,
	uint7,
	uuid7,
} from './filter-preview-appearance-common.js';
const ranges = {
	'invert-rgb-v1': {},
	'solarize-rgb-v1': {},
	'brightness-contrast-v1': {
		brightness: [-100, 100],
		contrast: [-100, 100],
	},
	'black-white-rgb-v1': { strength: [0, 100] },
	'threshold-luma-v1': { level: [0, 255] },
	'posterize-rgb-v1': { levels: [2, 32] },
	'box-blur-v1': { radius: [0, 32] },
	'levels-rgb-v1': {
		input_black: [0, 254],
		input_white: [1, 255],
		gamma_hundredths: [10, 999],
		output_black: [0, 255],
		output_white: [0, 255],
	},
	'exposure-srgb-v1': { stops_hundredths: [-800, 800] },
	'vibrance-rgb-v1': { amount_hundredths: [-100, 100] },
	'hue-saturation-rgb-v1': {
		hue_degrees: [-180, 180],
		saturation_hundredths: [-100, 100],
		lightness_hundredths: [-100, 100],
	},
	'add-noise-rgb-v1': { amount_hundredths: [0, 100], seed: [0, 65535] },
	'curves-rgb-v1': {},
};
export function validateDescriptor7(value) {
	if (!value || typeof value.kind !== 'string')
		throw fail7('InputSchema', 'Source7 descriptor');
	numericalEntry7({ profile_id: value.kind, codec_id: 'params-v1' });
	const fields = ranges[value.kind],
		extra =
			value.kind === 'curves-rgb-v1'
				? ['outputs']
				: value.kind === 'add-noise-rgb-v1'
					? ['monochrome', 'distribution']
					: [];
	shape7(value, ['kind', ...Object.keys(fields), ...extra]);
	for (const [key, [low, high]] of Object.entries(fields))
		uint7(value[key], high, low);
	if (
		value.kind === 'levels-rgb-v1' &&
		value.input_black >= value.input_white
	)
		throw fail7('InputSchema', 'Source7 ordered input endpoints');
	if (value.kind === 'curves-rgb-v1') {
		if (!Array.isArray(value.outputs) || value.outputs.length !== 5)
			throw fail7('InputSchema', 'Source7 five curves bytes');
		for (const n of value.outputs) uint7(n, 255);
	}
	if (
		value.kind === 'add-noise-rgb-v1' &&
		(typeof value.monochrome !== 'boolean' ||
			!['uniform', 'gaussian'].includes(value.distribution))
	)
		throw fail7('InputSchema', 'Source7 Noise fields');
}
function reference(value) {
	shape7(value, ['id']);
	uuid7(value.id);
}
function alias(value) {
	if (typeof value !== 'string' || !/^[a-z][a-z0-9_-]{0,63}$/.test(value))
		throw fail7('InputSchema', 'Source7 alias');
}
export function validateOperation7(value, target) {
	if (value === null) {
		if (target !== null) throw fail7('InputSchema', 'Source7 BASE target');
		return;
	}
	uuid7(target);
	const required = {
		'filter.add': ['op', 'layer_ref', 'ref', 'descriptor'],
		'filter.update': ['op', 'layer_ref', 'filter_ref', 'descriptor'],
		'filter.set_enabled': ['op', 'layer_ref', 'filter_ref', 'enabled'],
		'filter.remove': ['op', 'layer_ref', 'filter_ref'],
		'filter.reorder': ['op', 'layer_ref', 'filter_ref', 'to_index'],
		'filter.duplicate': ['op', 'layer_ref', 'filter_ref', 'ref'],
	};
	if (!value || !Object.hasOwn(required, value.op))
		throw fail7(
			'CapabilityUnsupported',
			'Source7 numeric FilterOp only; masks require Editor publication',
		);
	shape7(
		value,
		required[value.op],
		value.op === 'filter.add'
			? ['enabled', 'to_index']
			: value.op === 'filter.duplicate'
				? ['to_index']
				: [],
	);
	reference(value.layer_ref);
	if (value.layer_ref.id !== target)
		throw fail7('InputSchema', 'Source7 stable target');
	if (Object.hasOwn(value, 'filter_ref')) reference(value.filter_ref);
	if (Object.hasOwn(value, 'ref')) alias(value.ref);
	if (Object.hasOwn(value, 'enabled') && typeof value.enabled !== 'boolean')
		throw fail7('InputSchema', 'Source7 enabled bool');
	if (Object.hasOwn(value, 'to_index')) uint7(value.to_index, 256);
	if (Object.hasOwn(value, 'descriptor'))
		validateDescriptor7(value.descriptor);
}
export function canonicalOperation7(value) {
	// All strings in an accepted numeric FilterOp are printable ASCII typed
	// names, aliases, enum values or canonical UUIDs; incremental output is JCS.
	if (value !== null) validateOperation7(value, value?.layer_ref?.id);
	return encodeProgram(value, 4096);
}
export function geometry7(view, size) {
	shape7(view, ['region', 'level']);
	uint7(view.level, 20);
	if (!Array.isArray(view.region) || view.region.length !== 4)
		throw fail7('InputSchema', 'Source7 ROI');
	const [x, y, w, h] = view.region;
	uint7(x, size[0]);
	uint7(y, size[1]);
	uint7(w, size[0], 1);
	uint7(h, size[1], 1);
	if (x + w > size[0] || y + h > size[1])
		throw fail7('InputSchema', 'Source7 ROI outside canvas');
	const step = 2 ** view.level,
		x0 = Math.floor(x / step) * step,
		y0 = Math.floor(y / step) * step;
	const rw = Math.min(Math.ceil((x + w) / step) * step, size[0]) - x0,
		rh = Math.min(Math.ceil((y + h) / step) * step, size[1]) - y0;
	const width = Math.ceil(rw / step),
		height = Math.ceil(rh / step);
	if (width * height > 4194304)
		throw fail7('ResourceLimit', 'Source7 output pixels');
	return { region: [x0, y0, rw, rh], width, height, level: view.level };
}
