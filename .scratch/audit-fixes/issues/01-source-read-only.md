# Reject state/source overlap before writing
Status: done

Scope: audit of d36e21d; Task.md §3 source-read-only invariant (line 58).

Acceptance: startup must reject configured sources overlapping application state before chmod, lock creation, database initialization/recovery, or logs. Job creation must check proposed sources before creating state. Resolve existing symlink ancestors and parent components, including a missing state directory. Recheck the selected Job before Run history writes for long-lived Core callers.

Implementation: read-only startup guard over configured sources; Core `open_with_sources` used by CLI Job creation; prospective path resolution; Run guard before history writes.

Validation: public CLI tests first failed on source-tree mutation, then passed. Tests compare file bytes, directory entries and mtimes, and Unix state-directory permissions; cover existing configuration, fresh state, and symlink/parent-component aliases. All fixtures are temporary. Filesystem replacement by an adversarial concurrent process is not claimed to be eliminated.
