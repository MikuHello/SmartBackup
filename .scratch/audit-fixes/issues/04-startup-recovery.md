# Interrupt abandoned Runs on normal startup
Status: done

Scope: audit of d36e21d; Task.md §11.4 startup interruption recovery (line 593), preserving Dry Run read-only behavior.

Acceptance: after SIGKILL during capture, the first normal startup (including history list) marks the abandoned Run interrupted/process_interrupted with a completion timestamp. Live lock owners must not be interrupted. Dry Run must not modify state; completed history and Artifacts remain unchanged, and repeated startup is idempotent.

Implementation: recover after the application obtains its exclusive lock and opens state, only for writable startup. Remove recovery from Run. Address the associated P3 history-scan concern with a running-only query and a compatible partial index, created only on writable opens; recovery no longer sorts/deserializes all completed history.

Validation: Unix CLI test failed with running before the fix and passes after it, using a real captured fixture and SIGKILL. It verifies busy handling while the owner is alive, exact state-tree equality across Dry Run, public history list/show results, idempotence, and unchanged completed history/Artifact bytes.
