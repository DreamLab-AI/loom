# Published generation reload

<!-- lands with ontomark step 10 -->
The `reload_published_generation` binary (crate `loom-facade`) is the once-per-tick transition from a
**complete published bundle** to a **verified served bundle**. The example user
timer runs every five minutes. It is supplied but not installed or enabled.

The publisher must first produce canonical `data/.generation.json` covering all
six artefacts: scaffold/prose indexes, asserted/inferred Turtle, semantic RVDB and
its generation sidecar. Every digest and size must match. The semantic sidecar
must declare the same generation, bge-small-en-v1.5 and 384 dimensions. A legacy
four-file mirror is insufficient and is rejected before a restart. This binary
does not relabel/re-embed an old RVDB, download content, promote files, or attest
that upstream public-export privacy is correct.

The marker check is the one in `loom-facade::mirror` (`parse_marker_strict`, the strict
side of the reader the server itself uses); there is no second copy of it in Python.
The strict side refuses what the server's lenient reader would tolerate: a malformed
artefact entry, a missing or mistyped C3 field, an id that does not exactly name its commit, or a legacy
marker without a `generation`. Dirty C3 builds (`visionGraph@<sha>-dirty`) are
refused for activation: publish from a clean source checkout. The marker is accepted in either shape (ADR-141). An `ontomark build`
marker — the one the corpus build writes, versioned by the bundle manifest — carries `id: "visionGraph@<sha>"`,
`commit`, `content_digest`, `generated_at`, `class_count`, `page_count`,
`vocabulary_version`, `stale_after` and an artefact **list**; each C3 field is
required once `id` is present and the id must name its own commit. `content_digest`
identifies the source corpus the bundle was built from; it is carried, not compared,
and a declared value that differs from the recomputed artefact digest is discarded,
not refused.
What is served and proved is the artefact digest recomputed from the individually
verified files. A legacy mirror marker
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
qualification. It checks the disk again after each successful probe, including
the already-serving case, before atomically writing `state: served`. A failed command,
timeout or concurrent publication leaves pending state and a non-zero process exit;
the next tick retries. An already matching service is not restarted. No shell is
used to interpret the reload argument list; the command after `--` is run as given and
is killed if it outlives `--timeout`.

Exit status: `0` after printing `current` (already serving) or `reloaded`; `1` for a
refused bundle, a failed or timed-out restart, a service that never proves the bundle,
or a lock held by another tick; `2` for a usage error (a missing flag or command, or a
`--timeout` outside 1..300, default 60). The receipt is `{"content_digest", "generation",
"state"}`, written atomically with mode 0600 next to a `<receipt>.lock` file.

`reload_published_generation verify --data-dir DIR` runs the bundle check alone (no lock,
receipt, probe or restart), prints `{"content_digest", "generation"}` and exits `0` or
`1`. The publishing steps below use it before copying to HP.

## When to enable the timer

The precondition is a single event (PRD sovereign-corpus Q8): **the first clean
promotion of an `ontomark build` bundle** — a complete six-artefact
`.generation.json` in the bundle-manifest shape, promoted into `data/`, verified by
one manual tick whose receipt reads `served`, and serving a generation no older
than the raw vault it replaces. Until that has happened the timer stays off: the
previous blocker was that the published bundle (graph 2026-08-22, semantic
2026-08-17) was both mixed and staler than the corpus on disk, and an automatic
reloader cannot improve a bundle that should not be served at all.

Build the binary on the node (`cargo build --release -p loom-facade --bin
reload_published_generation`), run the manual tick first and read the receipt:

```bash
~/githubs/loom/target/release/reload_published_generation \
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

Isolated tests in `loom-facade` (`reload_published_generation`, `mirror_generation`,
`adr141_generation_and_ledger`)
exercise success, no-op, refusal and failure-state handling across both marker
shapes. Reloading is an implemented transition;
producing a valid new upstream bundle and activating it remain distinct work.

## Publishing a new generation

The timer reloads what is already in `data/`; it never builds or fetches. A node
therefore follows `visionGraph` only as fast as someone publishes. One publication
has three steps, and only the last touches HP.

<!-- lands with ontomark step 10 -->
**1. Build** from a clean `visionGraph` checkout with a released `ontomark`. Pin
`ontomark` to a release tag and checksum, never to a branch, and confirm
`ontomark --version` reports the release's CLI version. The release pin file
`deploy/ontomark-release.env` pins the published `v0.1.0-rc.1` Linux x86-64 asset;
its CLI version is `0.1.0`. The typed serving readers use the independent
`ontomark-compile` crate release with its reasoner disabled. The `vault` baked into agentbox images up to at least
2026-10-02 (0.1.0) predates the portable-records file and writes JSONL under the
database's name, `ontology-corpus.rvdb`, which `promote_vault_build` cannot use,
so do not use it. Install the released binary into a private directory (agentbox
bakes `ontomark` once its flake input moves to the tag):

```bash
source deploy/ontomark-release.env
mkdir -p ~/workspace/loom-publish/ontomark/bin
gh release download "$ONTOMARK_RELEASE_TAG" --repo "$ONTOMARK_RELEASE_REPO" \
  --pattern "$ONTOMARK_ASSET" --dir ~/workspace/loom-publish/ontomark
printf '%s  %s\n' "$ONTOMARK_SHA256" \
  "$HOME/workspace/loom-publish/ontomark/$ONTOMARK_ASSET" | sha256sum --check --strict
install -m755 ~/workspace/loom-publish/ontomark/"$ONTOMARK_ASSET" \
  ~/workspace/loom-publish/ontomark/bin/ontomark
SHA=$(git -C ~/workspace/visionGraph rev-parse --short=9 HEAD)
~/workspace/loom-publish/ontomark/bin/ontomark --repo ~/workspace/visionGraph \
  build --out ~/workspace/loom-publish/$SHA --with-rvdb --stats
```

`data/` in the output holds the Loom bundle, with `.generation.json` listing its
artefacts by path relative to `data/`.

**2. Promote and verify** in its own process:

```bash
cd ~/workspace/loom
CARGO_TARGET_DIR=~/workspace/loom-publish/loom-target \
  cargo build --release -p loom-vector-ruvector --bin promote_vault_build
~/workspace/loom-publish/loom-target/release/promote_vault_build \
  --data ~/workspace/loom-publish/$SHA/data
CARGO_TARGET_DIR=~/workspace/loom-publish/loom-target \
  cargo build --release -p loom-facade --bin reload_published_generation
~/workspace/loom-publish/loom-target/release/reload_published_generation verify \
  --data-dir ~/workspace/loom-publish/$SHA/data
```

**3. Copy to HP under the in-flight marker**, from the machinelearn host shell (HP
is reachable only from there). The agentbox workspace is
`/mnt/nvme/docker/volumes/multi-agent-docker_workspace/_data` on that host. Never
use `--delete`: `data/` also holds `ledger.jsonl`, the attestation chain.

```bash
SRC=/mnt/nvme/docker/volumes/multi-agent-docker_workspace/_data/loom-publish/$SHA/data
rsync -a "$SRC"/ john@10.10.10.1:githubs/loom/data.$SHA/
ssh john@10.10.10.1 "set -e; cd ~/githubs/loom
  rm -rf data.rollback; cp -a data data.rollback # previous generation + ledger
  touch data/.promotion-in-flight
  rsync -a --exclude .generation.json data.$SHA/ data/
  cp data.$SHA/.generation.json data/.generation.json.new
  mv data/.generation.json.new data/.generation.json
  rm data/.promotion-in-flight"
```

The marker is replaced last and atomically, and the in-flight file makes any tick
that runs during the copy refuse rather than reload a half-copied bundle. The next
tick, or a manual one, then restarts `loom` and writes `served`.
