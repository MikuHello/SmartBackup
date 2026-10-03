# Reject state/source overlap before writing
Status: done

Scope: audit of d36e21d; Task.md §3 source-read-only invariant (line 58).

Acceptance: startup must reject configured sources overlapping application state before chmod, lock creation, database initialization/recovery, or logs. Job creation must check proposed sources before creating state. Resolve existing symlink ancestors and parent components, including a missing state directory. Recheck the selected Job before Run history writes for long-lived Core callers.

Implementation: read-only startup guard over configured sources; Core `open_with_sources` used by CLI Job creation; prospective path resolution; Run guard before history writes.

Validation: public CLI tests first failed on source-tree mutation, then passed. Tests compare file bytes, directory entries and mtimes, and Unix state-directory permissions; cover existing configuration, fresh state, and symlink/parent-component aliases. All fixtures are temporary. Filesystem replacement by an adversarial concurrent process is not claimed to be eliminated.

## Review follow-up

Independent review of `d36e21d..1524c23` reopened this P1: the guard resolved `home` but configuration reads and directory creation still used its original spelling. Missing components followed by `..` could create directories within a source or bypass configuration discovery. The original validation did not cover those cases. See [05-state-path-consistency.md](05-state-path-consistency.md) for the corrective implementation and regression evidence; this entry is not evidence that the first fix fully closed the issue.
