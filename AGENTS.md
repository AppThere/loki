<!-- code-review-graph MCP tools -->
## MCP Tools: code-review-graph

**IMPORTANT: This project has a knowledge graph. ALWAYS use the
code-review-graph MCP tools BEFORE using Grep/Glob/Read to explore
the codebase.** The graph is faster, cheaper (fewer tokens), and gives
you structural context (callers, dependents, test coverage) that file
scanning cannot.

### When to use graph tools FIRST

- **Exploring code**: `semantic_search_nodes` or `query_graph` instead of Grep
- **Understanding impact**: `get_impact_radius` instead of manually tracing imports
- **Code review**: `detect_changes` + `get_review_context` instead of reading entire files
- **Finding relationships**: `query_graph` with callers_of/callees_of/imports_of/tests_for
- **Architecture questions**: `get_architecture_overview` + `list_communities`

Fall back to Grep/Glob/Read **only** when the graph doesn't cover what you need.

### Key Tools

| Tool | Use when |
|------|----------|
| `detect_changes` | Reviewing code changes — gives risk-scored analysis |
| `get_review_context` | Need source snippets for review — token-efficient |
| `get_impact_radius` | Understanding blast radius of a change |
| `get_affected_flows` | Finding which execution paths are impacted |
| `query_graph` | Tracing callers, callees, imports, tests, dependencies |
| `semantic_search_nodes` | Finding functions/classes by name or keyword |
| `get_architecture_overview` | Understanding high-level codebase structure |
| `refactor_tool` | Planning renames, finding dead code |

### Workflow

1. The graph auto-updates on file changes (via hooks).
2. Use `detect_changes` for code review.
3. Use `get_affected_flows` to understand impact.
4. Use `query_graph` pattern="tests_for" to check coverage.
5. **Documentation Sync**: When implementing or fixing any layout/rendering properties or import/export capabilities, update the living status registry at [docs/fidelity-status.md](docs/fidelity-status.md).

---

## Project conventions (read `CLAUDE.md` first)

[`CLAUDE.md`](CLAUDE.md) is the authoritative contributor guide (coding
conventions, the 300-line file ceiling, error-handling and i18n rules, the
workspace layout, and the Dioxus version pin). Follow it.

### Fix the cause, not the symptom

**Always implement the correct, root-cause fix rather than a quick patch.**
Diagnose *why* a problem happens before changing anything; fix it at the layer
where it belongs; do not paper over issues with sleeps, retries, magic numbers,
broad `#[allow]`s, swallowed errors, or disabled tests. If a true fix is out of
scope, implement the smallest honest stopgap, mark it `// TODO(<topic>):`, and
record it as tech debt — never present a workaround as a fix. Remove any
temporary debugging code before committing, and report partial/unverified work
honestly. See the "Engineering principles" section in `CLAUDE.md` for the full
expectation.

### Dependency patches & Dioxus

Local `[patch.crates-io]` crates are documented in
[docs/patches.md](docs/patches.md). Dioxus is pinned to an exact version
because the vendored `dioxus-native{,-dom}` patches are version-specific; to
upgrade it, follow **"Upgrading Dioxus"** in `docs/patches.md` — do not loosen
the pin or bump the version without re-vendoring the patches.

### Before a commit is "complete"

`cargo fmt --all` and `cargo clippy --workspace -- -D warnings` must both pass,
and `cargo check --workspace` must be clean.

