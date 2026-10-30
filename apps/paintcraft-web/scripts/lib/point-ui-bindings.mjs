// Pure observation predicate, shared byte-for-byte by browser wait and retrieval.
export function canonical(value) {
	return Array.isArray(value)
		? value.map(canonical)
		: value && typeof value === 'object'
			? Object.fromEntries(
					Object.keys(value)
						.sort()
						.map((key) => [key, canonical(value[key])]),
				)
			: value;
}
export function matchingPointPair(events, expected) {
	const normalized = (value) =>
		Array.isArray(value)
			? value.map(normalized)
			: value && typeof value === 'object'
				? Object.fromEntries(
						Object.keys(value)
							.sort()
							.map((key) => [key, normalized(value[key])]),
					)
				: value;
	const equal = (a, b) =>
		JSON.stringify(normalized(a)) === JSON.stringify(normalized(b));
	const {
		binding,
		operation,
		operationSha256,
		generation,
		pixels,
		since,
		target,
	} = expected;
	const view = { region: [0, 0, 2, 1], level: 0 };
	for (const event of events) {
		const request = event.request;
		if (
			event.sequence <= since ||
			event.workerId !== binding.workerId ||
			!request ||
			request.generation !== generation ||
			!equal(request.owner, binding.owner) ||
			request.draft !== binding.draft ||
			request.sourceSha256 !== binding.sourceSha256 ||
			request.target !== target ||
			!equal(request.operation, operation) ||
			!equal(request.view, view) ||
			request.render !== true
		)
			continue;
		const reply = events.find(
			(entry) =>
				entry.sequence > event.sequence &&
				entry.kind === 'frame' &&
				entry.workerId === binding.workerId &&
				entry.generation === generation &&
				equal(entry.owner, binding.owner) &&
				entry.draft === binding.draft &&
				entry.sourceSha256 === binding.sourceSha256 &&
				entry.target === target &&
				equal(entry.view, view) &&
				entry.operationSha256 === operationSha256 &&
				equal(entry.region, [0, 0, 2, 1]) &&
				entry.level === 0 &&
				entry.width === 2 &&
				entry.height === 1 &&
				equal(entry.pixels, pixels),
		);
		if (reply) return { request, reply };
	}
	return null;
}
