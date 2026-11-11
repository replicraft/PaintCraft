# Scoped native JPEG XR capability generations

T33 remains **partial**. The first shared consumer is the existing native Studio
JPEG XR copy-export path, with explicit CLI installation discovery. This is not
a general capability/grant service or new format/backend qualification.

The host verifies the installed helper and manifest against its exact bounded
writer contracts. Missing expected files are unavailable; filesystem, resource
and cancellation errors remain errors. Shared descriptive records include the
verified helper/manifest identities, application version and OS/architecture.
They neither authorize execution nor enable an unqualified writer.

Native keeps a process-local monotonic generation across dialog cancellation.
Only the newest refresh request can apply; an observed available→unavailable→
same-available cycle invalidates old tickets. The initial observation uses the
existing admitted editing executor and retires before encode admission. Queued
encoding binds the captured ticket and both verified identities before launch.
The same original admitted operation refreshes after encoding/decoded preview;
UI adoption checks the ticket and retained identity. The existing staged-save
preparation checkpoint repeats observation and retained-helper/owner checks
before publication. Existing document, path, destination, loss/bake and resource
protections remain. No extra root, pool or larger limit is introduced.

`paintcraft-cli capabilities --installation-jxr --json` selects the same scoped
record as the native export model. Default discovery retains its old
`capability_generation: "0"` shape. A local epoch is not a globally comparable CLI
counter; scoped records do not claim an exact application-binary registry.

This detects observed environment changes, not an unobserved external ABA or an
atomic filesystem/spawn/publication transaction. The retained verified helper
continues to check file identity and bytes. Availability does not dirty document
content, replace bake consent or imply release qualification.

## Current qualification

The first focused native run passed 39 Platform helper tests, five Formats ledger
tests, 13 desktop JXR tests and one actual CLI discovery test. One Formats and
nine desktop installed tests were explicitly ignored in that run. Strict native
all-target Clippy passed for Platform, Formats, desktop and CLI; existing pinned
third-party warnings remain separate. These are source/model/native checks,
not installed UI/helper acceptance.

The broader native checkpoint passes 2,643 tests across 144 suites with 146
existing or explicitly gated ignored cases, plus benchmark smoke execution. It
precedes the final three installed tests, removal of modal diagnostics and the
Japanese test-selector fix. Final builds and strict lint pass after those narrow
changes; Platform/Formats strict rustdoc also passes.

Actual copied-installation acceptance passes all eight selected top-level tests.
The positive selector exercises 14 English/Japanese export cases, eight fresh
round-trips and eight nested import scenarios. The remaining selectors verify
same-capacity admission, observed revocation and ABA, actual desktop/CLI record
equality, physical manifest removal during staging, stale successful results
before UI adoption, and manifest mismatch refusal before helper launch. Source,
destination and original lease checks pass; copied/source artifacts remain
unchanged. This uses the actual V10 helper and retained portable goldens, without
running an external C oracle or opening native windows.

The first installed attempt failed because a Japanese test query matched both a
disabled toolbar button and the active confirmation. The corrected selector
requires exactly one enabled button; application behavior and pixel assertions
are unchanged. That failure and two test-helper compiler failures remain
preserved. The final 23 scoped source files match before and after execution.
The aggregate receipt is
`docs/validation/t33-jxr-generation-root/root-final.json`
(SHA-256 `b43d5c69163a8e836352b1ab6985c45410dc62fa14b4fdfb8ce0bfcdb351e035`).
This is local native acceptance, not release or browser qualification.

## Still open in T33

Generation-aware discovery across other formats/backends, permission/grant
revocation, environment/registry/plugin changes, and all CAP-01–10 profiles remain
open. Unsupported mandatory features, decoder-only or unqualified writers,
GPU-exactness fallback, unavailable oracle and synthetic/release evidence retain
their existing restrictions. No plugin runtime, configurable grant toggle or
browser JPEG XR export is added by this slice.
