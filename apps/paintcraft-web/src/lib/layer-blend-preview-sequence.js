// Metadata ordering witness, NOT an execution/job/credit capability.

import { copy8, fail8 } from './layer-blend-preview-common.js';
import {
	validateAck8,
	validateCancel8,
	validateCancelled8,
	validateClose8,
	validateClosed8,
	validateComplete8,
	validateOpen8,
	validateOpened8,
	validatePatch8,
	validateView8,
} from './layer-blend-preview-protocol.js';

export class Protocol8Sequence {
	#open;
	#view;
	#close;
	#cancel;
	#opened = false;
	#id = 0;
	#token = 0n;
	#generation = 0n;
	#ordinal = 0;
	#pending = false;
	#terminal = true;
	#closed = false;
	#request(id) {
		if (this.#close || this.#closed || id <= this.#id)
			fail8('ConflictStaleBase', 'Protocol8 request order/closed');
	}
	#active() {
		if (!this.#open || !this.#view || this.#terminal || this.#close)
			fail8('ConflictStaleBase', 'Protocol8 no active operation');
	}
	open(value) {
		const open = validateOpen8(value);
		this.#request(open.id);
		if (this.#open) fail8('ConflictStaleBase', 'Protocol8 OPEN once');
		this.#open = open;
		this.#id = open.id;
		return open;
	}
	opened(value) {
		if (!this.#open || this.#opened || this.#close)
			fail8('ConflictStaleBase', 'Protocol8 unexpected OPENED');
		validateOpened8(value, this.#open);
		this.#opened = true;
	}
	view(value) {
		if (!this.#opened || this.#cancel)
			fail8('ConflictStaleBase', 'Protocol8 VIEW before joined/opened');
		const view = validateView8(value, this.#open);
		this.#request(view.id);
		if (
			!this.#terminal ||
			BigInt(view.token) <= this.#token ||
			BigInt(view.generation) <= this.#generation
		)
			fail8('ConflictStaleBase', 'Protocol8 monotonic new operation');
		this.#view = view;
		this.#id = view.id;
		this.#token = BigInt(view.token);
		this.#generation = BigInt(view.generation);
		this.#ordinal = 0;
		this.#pending = false;
		this.#terminal = false;
		return view;
	}
	patch(value) {
		this.#active();
		validatePatch8(value, this.#view, this.#open, this.#ordinal);
		if (this.#pending)
			fail8('ConflictStaleBase', 'Protocol8 one outstanding patch');
		this.#pending = true;
		// Caller must copy before ack(); this witness never owns the buffer.
	}
	ack(value) {
		this.#active();
		validateAck8(value, this.#view, this.#open, this.#ordinal);
		this.#request(value.id);
		if (!this.#pending)
			fail8('ConflictStaleBase', 'Protocol8 ACK without patch');
		this.#id = value.id;
		this.#pending = false;
		this.#ordinal++;
	}
	complete(value) {
		this.#active();
		validateComplete8(value, this.#view, this.#open);
		if (this.#pending || this.#ordinal !== value.patchCount)
			fail8('ConflictStaleBase', 'Protocol8 incomplete copy');
		this.#terminal = true;
	}
	cancel(value) {
		this.#active();
		validateCancel8(value, this.#view, this.#open);
		this.#request(value.id);
		this.#id = value.id;
		this.#pending = false;
		this.#terminal = true;
		this.#cancel = copy8(value);
		// Runtime must join the actual Rust call/image before reporting cancelled.
	}
	cancelled(value) {
		if (!this.#cancel || this.#close)
			fail8('ConflictStaleBase', 'Protocol8 unexpected CANCELLED');
		validateCancelled8(value, this.#cancel, this.#view, this.#open);
		this.#cancel = undefined;
	}
	close(value) {
		if (!this.#open)
			fail8('ConflictStaleBase', 'Protocol8 CLOSE before OPEN');
		validateClose8(value, this.#open, this.#token.toString());
		this.#request(value.id);
		// Close ACK object contains only fully validated immutable scalar metadata.
		this.#close = Object.freeze({
			...value,
			publicTicket: Object.freeze({ ...value.publicTicket }),
		});
		this.#id = value.id;
		this.#pending = false;
		this.#terminal = true;
	}
	closed(value) {
		if (!this.#close || this.#closed)
			fail8('ConflictStaleBase', 'Protocol8 unexpected CLOSED');
		validateClosed8(value, this.#close, this.#open);
		this.#closed = true;
		// No broker release here. Actual joined close + Main bytes0 are separate.
	}
}
