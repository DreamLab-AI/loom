# Published generation reload

`scripts/reload-published-generation.py` is the once-per-tick transition from a
**complete published bundle** to a **verified served bundle**. The example user
timer runs every five minutes. It is supplied but not installed or enabled.

The publisher must first produce canonical `data/.generation.json` covering all
six artefacts: scaffold/prose indexes, asserted/inferred Turtle, semantic RVDB and
its generation sidecar. Every digest and size must match. The semantic sidecar
must declare the same generation, bge-small-en-v1.5 and 384 dimensions. A legacy
four-file mirror is insufficient and is rejected before a restart. This script
does not relabel/re-embed an old RVDB, download content, promote files, or attest
that upstream public-export privacy is correct.

For the HP checkout layout, the supplied user service explicitly runs compose
`up --no-deps --force-recreate loom`; it does not restart the model, Agentbox or
other services. Adjust the reviewed absolute paths before installing it elsewhere.
The old image and generation must remain available for operator rollback. Run one
manual tick with the same arguments and inspect the receipt before enabling the
timer. No automatic rollback replaces a failed generation silently.

The transition refuses incomplete, hash-mismatched, indirect or in-flight
publication, serialises invocations using a lock, and writes a pending receipt
before reload. It verifies the same on-disk bundle again and polls the server for
the expected loaded digest, matching graph/semantic generation and valid embedding
qualification. Only then does it atomically write `state: served`. A failed command,
timeout or concurrent publication leaves pending state and a non-zero process exit;
the next tick retries. An already matching service is not restarted. No shell is
used to interpret the reload argument list.

On 2026-09-07, the live HP bundle exposes graph generation 2026-08-22 and semantic
2026-08-17; its generation API is older than the required identity contract.
Therefore **do not enable this timer yet**. Publish a matched, fully declared
semantic/graph bundle and deploy the verified server first. Eight isolated tests
exercise success, no-op, refusal and failure-state handling. Reloading is now an
implemented transition; producing a valid new upstream bundle and activating it
remain distinct work.
