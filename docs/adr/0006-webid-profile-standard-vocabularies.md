# Mint a full WebID profile; reuse standard vocabularies

At first capture, ns-os generates a complete, valid self-issued **WebID profile
document** (a named graph of quads): `<…#me> a foaf:Person ; foaf:name "<captured>"`
plus the SOLID/PIM terms that make it a recognisable pod profile. The `#me` node
is the identity Fragment.

## Context

This commits us to **reusing standard vocabularies** (FOAF, SOLID/PIM, RDF/RDFS,
Dublin Core) rather than inventing our own wherever a standard exists —
semantic-web interoperability is the entire point. A minimal `ns:` vocabulary is
used only for concepts with no good standard (e.g. device specifics).
