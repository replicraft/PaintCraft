// Independent tiny-fixture full-square sums, with exact BigInt rationals.
// No product kernel imports or separable/intermediate-rounded computation.
import assert from 'node:assert/strict';

const nearest = (n, d) => {
	assert.ok(n >= 0n && d > 0n);
	return Number((2n * n + d) / (2n * d));
};
export function boxPlane(
	rgba,
	width,
	height,
	radius,
	region = [0, 0, width, height],
	origin = [0, 0],
) {
	assert.ok(Number.isInteger(radius) && radius >= 0 && radius <= 32);
	assert.ok(
		width > 0 &&
			height > 0 &&
			width <= 256 &&
			height <= 256 &&
			rgba.length === width * height * 4,
	);
	const [left, top, rw, rh] = region,
		[ox, oy] = origin;
	assert.ok(
		left >= ox &&
			top >= oy &&
			rw > 0 &&
			rh > 0 &&
			left + rw <= ox + width &&
			top + rh <= oy + height,
	);
	const output = [];
	for (let y = top - oy; y < top - oy + rh; y++)
		for (let x = left - ox; x < left - ox + rw; x++) {
			const p = (y * width + x) * 4;
			if (radius === 0) {
				output.push(...rgba.slice(p, p + 4));
				continue;
			}
			const weights = new Map();
			// Enumerate the true square, including duplicated CLAMP samples. The
			// multiplicities merely avoid repeating identical integer products.
			for (let dy = -radius; dy <= radius; dy++)
				for (let dx = -radius; dx <= radius; dx++) {
					const index =
						(Math.max(0, Math.min(height - 1, y + dy)) * width +
							Math.max(0, Math.min(width - 1, x + dx))) *
						4;
					weights.set(index, (weights.get(index) ?? 0n) + 1n);
				}
			let alpha = 0n;
			const color = [0n, 0n, 0n];
			for (const [index, copies] of weights) {
				const a = copies * BigInt(rgba[index + 3]);
				alpha += a;
				for (let c = 0; c < 3; c++)
					color[c] += a * BigInt(rgba[index + c]);
			}
			output.push(
				...color.map((sum) => (alpha ? nearest(sum, alpha) : 0)),
				nearest(alpha, BigInt((2 * radius + 1) ** 2)),
			);
		}
	return Buffer.from(output);
}
export function spatialMix(incoming, candidate, coverage) {
	assert.ok(Number.isInteger(coverage) && coverage >= 0 && coverage <= 255);
	if (coverage === 0) return [...incoming];
	if (coverage === 255) return [...candidate];
	const p = BigInt(255 - coverage),
		q = BigInt(coverage);
	const pa = p * BigInt(incoming[3]),
		qa = q * BigInt(candidate[3]),
		weight = pa + qa;
	const rgb = [0, 1, 2].map((c) =>
		weight
			? nearest(
					pa * BigInt(incoming[c]) + qa * BigInt(candidate[c]),
					weight,
				)
			: nearest(p * BigInt(incoming[c]) + q * BigInt(candidate[c]), 255n),
	);
	return [...rgb, nearest(weight, 255n)];
}
