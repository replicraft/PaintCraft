# Prepared MSP export protocol7

Source implementation pending qualification and actual browser integration. This
private profile does not widen protocols4/5/6 or their original algorithm lists.

Request448 and response608; Q07/R07 magics, version7. Common prefix layouts remain
unchanged. Region64 at request256/response416 uses sourcecontainer6/regioncontract2.
Program128 at request320/response480 binds the distinct required pairs, all stored
and enabled instance counts, unique prepared keys, five resource facts and both
required/resolved-subset SHA256 values. Exact framing is in
`docs/architecture/PREPARED_WORKER_CAPABILITIES.md`.

Actual frozen source length finalizes the maximum of parser+registry-control and
compile/evaluation/encoding phases. The explicit256-byte control-copy allowance is
one decoded/captured program context plus one echoed context, not the wire delta
from protocol4; packet/header/region framing is covered by the existing1MiB parser
control slot. The wire delta from4 is192 bytes per request/response.

The worker checks outer SourceSHA, creates one opaque inner-integrity witness,
resolves the required subset through a separately priced registry before retaining
source data, then adopts through that same witness. No extra inner SHA pass is
added for negotiation. It recomputes contexts and actual inventory before evaluation,
uses one shared parent compilation/regional work counter and prepares once for the
whole frame. Metadata omission, mask intent and editable-filter bake consent stay
independent captured authorities. Unknown required implementations/pricing reject
even disabled/default intentions. Old eight-profile documents keep minimal4/5/6.

Caps and output conventions are unchanged. Native producer/consumer tests are
same-ABI controls; MainWasm32/WorkerWasm32 facts require actual browser validation.
MSP still accepts only evaluated opaque black/white, without implicit quantization.
