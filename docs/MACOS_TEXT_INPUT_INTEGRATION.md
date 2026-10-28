# macOS text input integration investigation

The experimental owned window already consumes AppKit pointer records. Keyboard
and IME delivery still need an ordered native source; a local key boundary does
not authenticate a later winit text event. Normal startup remains unchanged.

## Public API candidate

Apple provides [`NSTextInputContext`](https://developer.apple.com/documentation/appkit/nstextinputcontext?language=objc)
with explicit activation and event handling, and the
[`NSTextInputClient`](https://developer.apple.com/documentation/appkit/nstextinputclient?language=objc)
protocol for a custom text view. This offers a candidate separate text client
without modifying winit's private view class.

An isolated local AppKit probe on 2026-10-27 attached a retained, zero-frame
`NSView` text client to a render view, created an input context for that client,
and activated it explicitly. Passing one fixed synthetic ASCII key event directly
to `handleEvent:` produced exactly one `insertText:replacementRange:` callback
containing `a`. The render view remained the window's first responder. The native
window closed and the process returned success. Source, executable, output and
hashes were retained under the local temporary validation directory.

This proves that specific public API interaction. The probe did not use winit,
install a local event monitor, inject an OS keyboard event, change the input
language or test Japanese composition. It does not establish an integrated or
qualified text-input implementation.

## Constraints on a production implementation

- Retain the client, context and native view for exactly the installed source
  lifetime. Remove observation and deactivate the context before releasing them.
- Preserve actual key and text callbacks as separate observations, with their
  native payload and checked source order. Do not synthesize text from key codes
  or associate deferred framework events by matching strings or callback timing.
- Keep input-context interpretation, native event dispatch and GUI execution
  outside the delivery/receiver locks. Reentrant callbacks must retain the
  installation identity and cannot expose a partially validated text frame.
- Bound and admit copied UTF-16, marked text and callback storage before accepting
  it. Preserve native range units and validate surrogate and range boundaries.
- Bind the editor's text target at the ordered semantic composition-begin edge,
  after earlier pointer actions and service barriers finish. Native capture
  carries a composition identity, not authority over a last-frame widget.
- Retire a binding on target, session, modal or source loss. A late result cannot
  select a replacement widget. Native composition end and policy cancellation
  remain distinct observations.
- Preserve native menu shortcuts, key equivalents, input-source changes, native
  chrome and candidate-window behavior. The API probe did not test these paths.
- Publish real caret geometry through the owned frontend. Candidate queries and
  character ranges must not claim document text or positions that the client
  does not actually own.

The locked winit 0.30.13 implementation already interprets keys in its private
view and implements the text-client protocol there. Merely adding another input
context would not prevent duplicate text. A production integration must own the
chosen keyboard/text path and prove that its framework mirrors cannot publish.
The [public macOS extension documentation](https://docs.rs/winit/0.30.13/winit/platform/macos/index.html) also says to obtain `sharedApplication`
after creating the event loop; constructing a replacement application before
winit is not a supported integration shortcut.

Next qualification must include an actual owned winit window, queued
click/compose/result ordering, Japanese input, selection and non-BMP text,
candidate placement, shortcuts, focus loss and teardown. The comprehensive native
input acceptance matrix remains in
[the integration spike](temp/paintcraft-implementation-spec-v5-hyperfine/docs/architecture/NATIVE_INPUT_INTEGRATION.md).
