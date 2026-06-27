# did:key self-sovereign identity

The user's identity is anchored to a `did:key` DID derived from a locally
generated Ed25519 keypair (seeded from `RDRAND`). The identity Fragment's IRI is
this DID. There is no DNS, authority, or ledger — the identity is fully
self-owned, which fits "the machine is your pod".

## Considered Options

- **https WebID under a per-install local authority** — rejected (less
  self-sovereign, needs a placeholder authority).
- **urn:** — rejected (not a valid http(s) WebID, not dereferenceable).

## Consequences

SOLID is https/WebID-centric, so interop later needs a DID→WebID bridge via
`owl:sameAs` (a named future task). The private key is generated and persisted on
the machine, raising an at-rest security concern (see ADR-0008 / roadmap).
