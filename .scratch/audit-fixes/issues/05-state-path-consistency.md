# Use one resolved state path for checks, reads and writes
Status: done

Scope: P1 follow-up from independent review of d36e21d..1524c23; Task.md:58 source-read-only invariant. Supersedes the incomplete path handling in 5f50380.

Root cause: startup compared a resolved path but scanned jobs and called create_dir_all using the original spelling. A missing component later cancelled by `..` was therefore either created inside a source or prevented job discovery before initialization.

Acceptance:
- New Job with home `source/new/../../state` may initialize the resolved state outside source, but must not create `source/new` or modify source entries, bytes or mtimes.
- Existing overlapping Job accessed through home `source/missing/../state` must be rejected before any mutation, including state permission changes.
- Configuration reads, lock creation, SQLite and logs must use the same resolved home as the guard, for normal startup and Dry Run.
- Keep the existing writable-home symlink rejection; normalization must not silently accept a linked home.

Implementation: all three App opening APIs now enter one startup path. It resolves home before proposed/configured-source checks and uses that PathBuf throughout initialization. Writable resolution checks the final directory component for a symlink before canonicalizing it.

Validation: both reported regressions were observed failing against the prior implementation, then passed after correction. CLI tests cover startup history/job/config/run/dry-run commands, source-tree byte/entry/mtime equality and Unix permissions, safe resolved-home creation and Dry Run state equality, plus direct and missing-component aliases of linked homes. All fixtures are temporary.

Limit: this is path-consistency protection, not a claim to eliminate hostile concurrent filesystem replacement. Cross-platform execution results must be reported separately from configured CI coverage.
