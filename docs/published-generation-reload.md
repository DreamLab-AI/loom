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

The marker is accepted in either shape (ADR-141). A `vault build` marker — the
one the sovereign-corpus build writes — carries `id: "visionGraph@<sha>"`,
`commit`, `content_digest`, `generated_at`, `class_count`, `page_count`,
`vocabulary_version`, `stale_after` and an artefact **list**; each C3 field is
required once `id` is present, the id must name its own commit, and a declared
`content_digest` that disagrees with the recomputed artefact set is refused even
when every individual file hashes correctly. A legacy mirror marker
(`generation: "<ISO stamp>"` and an artefact **map**) is still accepted, because
the live node carries one until the first vault-build promotion.

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

## When to enable the timer

The precondition is a single event (PRD sovereign-corpus Q8): **the first clean
promotion of a `vault build` bundle** — a complete six-artefact
`.generation.json` in the vault-build shape, promoted into `data/`, verified by
one manual tick whose receipt reads `served`, and serving a generation no older
than the raw vault it replaces. Until that has happened the timer stays off: the
previous blocker was that the published bundle (graph 2026-08-22, semantic
2026-08-17) was both mixed and staler than the corpus on disk, and an automatic
reloader cannot improve a bundle that should not be served at all.

Run the manual tick first and read the receipt:

```bash
python3 ~/githubs/loom/scripts/reload-published-generation.py \
  --data-dir ~/githubs/loom/data \
  --receipt  ~/.local/state/loom/reload.json \
  --url      http://127.0.0.1:8084 \
  -- /usr/bin/docker compose -f ~/githubs/loom/deploy/compose.profile-a.yml \
       up -d --no-deps --force-recreate loom
jq . ~/.local/state/loom/reload.json      # expect {"state": "served", …}
```

Then, and only then, install and enable the supplied user units on the HP node:

```bash
install -Dm644 ~/githubs/loom/deploy/loom-generation-reload.service \
  ~/.config/systemd/user/loom-generation-reload.service
install -Dm644 ~/githubs/loom/deploy/loom-generation-reload.timer \
  ~/.config/systemd/user/loom-generation-reload.timer
systemctl --user daemon-reload
systemctl --user enable --now loom-generation-reload.timer
loginctl enable-linger "$USER"            # ticks without an open session
systemctl --user list-timers loom-generation-reload.timer
journalctl --user -u loom-generation-reload.service -n 50
```

To stop it again: `systemctl --user disable --now loom-generation-reload.timer`.

Thirteen isolated tests exercise success, no-op, refusal and failure-state
handling across both marker shapes. Reloading is an implemented transition;
producing a valid new upstream bundle and activating it remain distinct work.
