# punkos

A from-scratch, bare-metal Rust operating system that abandons Windows/POSIX
conventions. It treats the machine as the user's SOLID **pod** and second brain:
everything the user (and the machine itself) knows is stored as **Fragments** in
an RDF-native graph, interoperable with the semantic web.

## Language

**Fragment**:
The atomic, addressable unit of stored information — one piece of content with a
stable IRI and a type. The only stored entity; everything is a fragment.
_Avoid_: note, file, document, record.

**Relation**:
A typed, directed edge between fragments (or from a fragment to a literal),
realized as an RDF predicate.
_Avoid_: link, tag, association.

**Idea**:
An emergent, named cluster or view over a connected set of fragments — never a
stored container.
_Avoid_: folder, notebook, project, collection.

**Capture**:
The act of turning a thought into a Fragment via a dialog that poses a question
(the first being _"Who are we?"_).
_Avoid_: input, entry, save.

**Pod**:
The user's personal store, which on punkos _is_ the local quad store itself (the
SOLID pod — there is no separate database).
_Avoid_: database, drive, disk.

**Quad store**:
The canonical knowledge store — subject, predicate, object, and named-graph —
term-interned (integer-ID'd terms) for footprint and performance.

**Identity**:
The self-sovereign `#me` Fragment — a `foaf:Person` named by a `did:key` DID —
that is the root of the pod and the lead acid bubble on screen.

**Device**:
A piece of the machine's own hardware (currently a storage device), surfaced as a
Fragment so the OS is self-describing. Linked to the Identity via `pim:storage`.
_Avoid_: drive, disk, volume, mount.
