# Uniform fragment graph (no hierarchy)

The data model has exactly one atomic stored type — the **Fragment** (stable
identity + content + type) — connected by typed **Relations** into a graph. An
**Idea** is an emergent, named cluster over connected fragments, *not* a stored
container.

## Context

This is the defining product decision. We explicitly **reject** the
folder/document/hierarchy model (a Windows/POSIX convention) in favour of a flat
graph, because it maps 1:1 onto RDF triples and lets ideas be relationships rather
than containers. The explicit "no" to hierarchy is as important as the "yes" to
the graph.
