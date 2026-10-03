# Verify engine integrity at every launch
Status: done

Scope: audit of d36e21d; Task.md §8 engine integrity before invocation (line 405).

Acceptance: replacing the configured engine during source capture must fail with `engine_hash_mismatch`, record a failed Run, and publish no Artifact. All engine invocation paths must share the check.

Implementation: validate the pinned SHA-256 immediately before `Command::spawn` in the common engine entry point. Existing early preflight checks remain for prompt failure.

Validation: Unix CLI regression pauses the application during capture, atomically replaces its temporary copy of real 7-Zip with a runnable copy containing appended bytes, and resumes. Before the fix it incorrectly completed successfully; after the fix it fails closed and history records the reason. Source fixture bytes remain unchanged. The process helper always reaps the fixture child, including on assertion failures.

Limit: this closes the long capture/prior-operation gap. Hashing a pathname followed by spawning that pathname is not an atomic operating-system operation; adversarial replacement in that final interval is not claimed to be prevented.
