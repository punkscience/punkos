# RDF-quad-native core store

The kernel's knowledge store **is** a quad store (subject–predicate–object +
named-graph/context). A Fragment is a convention over quads; each Fragment has a
stable IRI. We use an efficient **term-interned** quad store (integer-ID'd terms,
not raw strings) for footprint and performance. Quads, not triples, so SOLID
named-graphs and provenance work.

## Considered Options

- **Native Rust structs, RDF only at the boundary** — rejected: the
  semantic-web-ness would live at the edge and the native model could drift from
  the RDF model.

## Consequences

The pod and the store are the same thing — no impedance mismatch. Scalar content
lives as literal-valued quads; binary media are blobs addressed by IRI with
descriptive quads.
