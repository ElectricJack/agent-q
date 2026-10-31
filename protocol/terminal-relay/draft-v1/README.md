# AQ-owned terminal relay candidate, draft v1

Review candidate: these documents alone enable no route, capability, worker
support or renderer. A separately qualified optional server adapter and CLI
candidate consume them; production/product integration remains gated. Existing published session protocol
and human API 0.1.0 bytes remain unchanged.

`aq.terminal-relay/1` adds explicit controller fence/ack/revoke, fresh-capture,
resize receipt, human renew/release/status, bounded node poll/upload/ack envelopes.
All existing identities, leases, literal commands, frames and snapshots use the
exact shared 0.1.0 types. Central Peer is independent; runtime central support
requires an independent authority adapter and is currently unsupported.

`aq.terminal-input-epoch/1` is a separately negotiated input mode. A terminal
registration chooses finite-v1 or epoch mode explicitly; epoch terminals refuse
the finite input endpoint. The frozen InputRequest is nested unchanged in the
new versioned request. The server mints one epoch per exact current controller
lease/generation; input IDs must follow the canonical UUIDv8 derivation below.
Keep exact receipts for the latest 64 input sequences. Older sequences/epochs
refuse unconditionally, including exact retries; changed input is never newly
accepted after retirement. Unresolved delivery_unknown prevents retirement and
further input, survives epoch/controller/restart changes and requires a fresh
server-registered capture resolution. A repeated old operation can return its
retained historical disposition, never dispatch again. Worker-side durable
retirement and injection-before-call persistence remain WRK14 qualification.

Canonical UUID derivation version1: SHA256 over the UTF-8/ASCII bytes
`aq.terminal-input-epoch/1` followed by one zero byte, the 32 decoded SHA256 bytes
of recursively key-sorted compact canonical TerminalIdentity JSON, 16 raw epoch
UUID bytes, 16 raw lease UUID bytes, controller generation u64 big endian and
input sequence u64 big endian. Take the first16 digest bytes; replace the high
nibble of byte6 with8 and the high2 bits of byte8 with binary10. Expiry is not an
input: renewing/replacing a lease creates a different generation/lease, while
one frozen epoch's identity remains stable. `uuid-vectors.json` includes three
independently computed SHA256 vectors, checked by Rust tests. SHA256 collision
resistance is assumed, not mathematical uniqueness.

Other mutation IDs (watch/controller renew/capture/resize) remain finite and are
never silently pruned. The pure ledger caps1,024 total retained mutation IDs and
1MiB encoded state; finite mode therefore has a session-lifetime quota. Epoch
mode retires only safe completed input records; unresolved records are pinned.
These are bounded qualification profiles, not long-lived capacity promises.
Normalized durable receipt/highwater storage, non-input replay horizons, production
authorization joins, encryption, pools and CLI transport require separate tests.

Schemas describe shape; runtime validators additionally enforce exact capabilities,
peer identity, digests, nil IDs, monotonic state, deadlines, counts and wire bytes.
Consumers must reject unknown required versions and unknown mutation fields.
A fence in an outgoing response is not an acknowledgement. Output ACK means
received, never read by a person. A snapshot barrier remains a peer assertion until
actual target terminal/renderer capture ordering is qualified; snapshot text alone
is not a generic terminal renderer state.

Rebuild: `cargo run -p aq-terminal-relay --example publish -- protocol/terminal-relay/draft-v1`.
Check: `cargo test -p aq-terminal-relay` (no database/provider/network services).

The AQ-owned RelayPoll has explicit peer, cursor, bounded hold and separate active
subscription/controller counts. It does not modify or reinterpret the frozen
NodeTerminalPoll. The server verifies actual live work before holding, rather than
trusting the advertised counts. Upload acknowledgements include explicit dropped
output gaps. Correlation exchange IDs are not mutation replay authority: enclosed
immutable operation/receipt/fence/capture IDs carry their own replay semantics.
The canonical encrypted ledger persists the exact sealed TerminalPolicy; smaller
input/pending/frame/replay budgets and output-only mode remain binding.

The metadata-only `/v1/terminal-relay/control-upload` route is a strict subset of
PeerUpload: frames, snapshots and gaps must all be empty. It uses reserved control
capacity. `/v1/terminal-relay/upload` uses read/data capacity. Adapters must
prioritize ACKs/receipts on control-upload; a cursor never substitutes for them.

The same deterministic public artifacts are included inside the core and portable
crate archives. Regenerate explicitly with `cargo run -p aq-terminal-relay
--example publish -- protocol/terminal-relay/draft-v1 --crate-artifacts`; drift
tests check the workspace publication and packaged copies. Typed bounded decode
and validation remain mandatory; schema validation does not grant authority.
