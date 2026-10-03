# Apply filters to single-file sources
Status: done

Scope: audit of d36e21d; Task.md §9 filter precedence and built-in exclusions.

Acceptance: a single `secret.env` source is excluded by `*.env`; a single `.DS_Store` source is excluded by the built-in rule. Preview and Artifact contents agree. Explicit `!` includes still override exclusions; empty filtered snapshots retain no-change behavior.

Implementation: only directory roots retain the alias-anchor exemption; file roots use the same resolved filter decision as other entries.

Validation: public CLI test failed with preview files=1 before the fix; now checks both exclusion cases, actual archive listings, explicit reinclusion, no-change skipping, and unchanged source bytes with real 7-Zip.
