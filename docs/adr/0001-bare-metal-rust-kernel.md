# Bare-metal Rust kernel

punkos is a bare-metal Rust kernel: a boot protocol hands control directly to our
kernel, which talks to hardware itself, with no host OS underneath. We chose this
over building a custom userland on a minimal Linux because it is the purest
expression of the project's goals — abandoning POSIX/Windows conventions, lowest
possible footprint, high performance.

## Consequences

Networking, TLS, an RDF store, and a web browser all become from-scratch efforts.
"Browse the web" and full SOLID networking are explicitly **years** out, not
months. We accept this trade-off; the foundation decision is revisited at the
web-browsing boundary (see roadmap).
