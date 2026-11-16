# Published generation reload

<!-- lands with ontomark step 10 -->
> Promotion (building, validating, installing and recording a generation) is documented in
> [§ Promotion and rollback](#promotion-and-rollback) below; this binary only reloads what is
> already installed.

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
timer. The reload tick never rolls back; the start-up fallback (see § Start-up fallback) is a
separate, loudly reported mechanism.

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
`deploy/ontomark-release.env` pins the published Linux x86-64 asset (tag and sha256; today
`v0.9.0`). Read the file, not this document, for the current pin. The typed serving readers use the independent
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

## Promotion and rollback

Promotion is the explicit, recorded step that puts a built bundle in service. It is
`promote_generation` (crate `loom-facade`), run nightly by `deploy/loom-nightly-promotion.sh`.

```bash
promote_generation --bundle DIR --data-dir DIR --source-repo DIR \
    [--log FILE] [--promoted-by NAME] [--allow-rollback]
```

`--bundle` is the `data/` directory `ontomark build` wrote (after `promote_vault_build`);
`--data-dir` is the directory the service reads; `--source-repo` is a visionGraph checkout.

**What it does, in order.** It takes a lock beside the log, validates the bundle without touching
`--data-dir`, compares it with what is installed, installs it, then logs it.

- Validation: the strict reload check (six artefacts, hashes and sizes, semantic sidecar,
  `visionGraph@<commit>` id), an `ontomark build` marker that declares its source commit, the
  scaffold and prose indexes and both Turtle files parse, and the commit exists in `--source-repo`.
  The semantic database is not opened (redb mutates it on open).
- Comparison: the same generation already installed prints `current <id>`, exits 0 and writes
  nothing. A bundle whose commit is a strict ancestor of the installed one is refused as older.
  So is one that cannot be ordered (the installed marker is legacy or its commit is not in the
  repository). `--allow-rollback` overrides both.
- Install: the existing `BundlePromoter` protocol: `.promotion-in-flight` sentinel, each artefact
  written beside as `<name>.incoming` and renamed, the marker renamed last. The marker adds
  `promoted_at` (UTC), `promoted_by` and `previous_generation`. The bundle directory is not modified.
- Log: one JSON line appended and fsynced.

Exit status: `0` after `promoted <id>` or `current <id>`; `1` for a refusal or failure; `2` for usage.
A refusal leaves the installed generation untouched. If the swap itself fails part-way (disk full),
the directory is left uncommitted, which the service and the reload tick both refuse; run
`promote_generation` again with a good bundle to repair it.

**Where the log is.** `<data-dir>/promotions.jsonl` unless `--log` says otherwise (the nightly uses
the default). Fields: `generation`, `source_repository`, `source_remote`, `commit`, `built_by`,
`promoted_at`, `promoted_by`, `previous_generation`, `content_digest`, `bundle`.

```bash
tail -n 3 ~/githubs/loom/data/promotions.jsonl | jq .
curl -s http://127.0.0.1:8084/health | jq '.generation | {id, promoted_at, promoted_by, previous_generation}'
```

**When it takes effect.** The running service never swaps bundles; it reports the generation it
loaded, and shows `serving_bundle.disk_matches_loaded: false` while a promoted one waits. The reload
timer restarts it within five minutes. Enable both timers (below), or run the reload tick by hand.

**Rollback.** Every line of the log names the `bundle` directory it was installed from, and the
nightly keeps the newest five under `~/.local/state/loom/bundles/<sha>/`. To go back to the
generation named in a line's `previous_generation`, find that id's bundle in the log and promote it
again with the flag, then reload:

```bash
BUNDLE=$(jq -r 'select(.generation=="visionGraph@<sha>") | .bundle' ~/githubs/loom/data/promotions.jsonl | tail -n 1)
~/githubs/loom/target/release/promote_generation --bundle "$BUNDLE" \
    --data-dir ~/githubs/loom/data --source-repo ~/githubs/visionGraph \
    --promoted-by "$USER" --allow-rollback
systemctl --user start loom-generation-reload.service     # or wait for the timer
```

The rollback is itself logged (`promoted_by` is you, `previous_generation` is the bad one). The
nightly will promote the newest commit again the next night; stop it first with
`systemctl --user disable --now loom-nightly-promotion.timer` if the rollback is meant to last.
A bundle that has been pruned cannot be re-promoted; rebuild it from its commit.

**The nightly** (`deploy/loom-nightly-promotion.sh`, with a user service and timer at 03:15 plus up
to ten minutes of delay) fetches visionGraph, resolves `origin/HEAD`, and exits 0 with `current` if
that generation is already installed and verifies. Otherwise it builds from a clean detached clone
with the `ontomark` pinned in `deploy/ontomark-release.env` (downloaded once, sha256-verified, cached
under `~/.local/state/loom/ontomark/<tag>/`) and the flags below, builds the semantic database with
`promote_vault_build`,
renames the finished bundle to `~/.local/state/loom/bundles/<sha>` and runs `promote_generation`. The
build happens outside the data directory, so the service never sees a half-built bundle. It does not
restart the service.

**Build flags.** They are those of visionGraph's publish workflow (`.github/workflows/publish.yml`
on `origin/main`, step "ontomark build → artefacts + content staging"; no other workflow there
calls ontomark), plus one Loom addition:

```text
before: ontomark --repo CLONE build --out OUT --with-rvdb --stats
after:  ontomark --repo CLONE build --vault all --out OUT --publish-out WORK/publish \
            --with-markdown-mirror --certificate-out OUT/certificate --stats --with-rvdb
```

- From the publish workflow: `--vault all`, `--out`, `--publish-out`, `--with-markdown-mirror`,
  `--certificate-out` (below `--out`, as in the workflow) and `--stats`. Only the paths differ: a
  private output directory, and the staging tree beside the bundle so it is not retained.
- Loom addition: `--with-rvdb`. The bundle check requires `ontology-corpus.rvdb` and its sidecar and
  `promote_vault_build` makes them from the records this flag emits; the publish workflow produces
  neither. It calls the embedder (`ONTOMARK_EMBED_ENDPOINT`, default
  `http://192.168.2.132:9997/v1/embeddings`).
- `--repo` only selects the clean clone. The workflow's `ontomark validate --vault all` and
  `ontomark repair bodies --vault all --check` are gates on the source, left to visionGraph's CI.
- The served bundle is unchanged by the new flags. Built both ways on visionGraph `a5b754af`, the
  marker's `class_count` (9595), `page_count`, `content_digest` and `ontology_digest` are identical, and
  the shared artefacts differ only in the build-time stamp. `--vault all` only widens the `publish/`
  staging to `working/`'s public pages (the ontology projections stay on `knowledge/`), and the nightly
  discards that staging. What the change buys is that the invocation is the published one.
- Cost: a retained bundle directory is now about 0.9 GB (markdown mirror and certificate included;
  `data/` is about 160 MB of it), so about 4.5 GB at the default; `LOOM_KEEP_BUNDLES` (default 5) bounds it.

Checked on 16 November 2026 with the real pinned `ontomark` v0.9.0 (asset sha256 verified against
`deploy/ontomark-release.env`) on a clone of visionGraph `a5b754af`: the build took under two minutes;
`promote_vault_build`, `promote_generation` into an empty data directory and
`reload_published_generation verify` all succeeded. The Loom installs the 21 artefacts the marker lists;
the certificate, markdown mirror and staging tree stay in the retained bundle, not in `data/`.

## Start-up fallback

If the generation in `data/` fails to load at start (a missing or mis-hashed artefact, an unreadable
marker, a promotion left in flight), the service serves the most recent retained previous generation
that validates instead of refusing, and says so. If none validates, it refuses to serve as before.

- **Where from.** The bundles the nightly keeps, `~/.local/state/loom/bundles/<commit>/data` (the
  newest `LOOM_KEEP_BUNDLES`, default 5). The compose files mount that directory read-only at
  `/app/retained-bundles` (`LOOM_BUNDLES_DIR` overrides the host path) and set
  `LOOM_RETAINED_BUNDLES_DIR`. Without that variable there is no fallback.
- **Which.** `data/promotions.jsonl` is read newest first, skipping the generation that failed (when
  its marker is too damaged to name it, nothing is skipped, so the newest logged generation may be
  served from its intact retained copy); the
  first whose retained bundle passes the checks promotion applies (see above, except that the source
  commit is taken on the log's word, as the container has no checkout) is served. A directory the log
  does not name is never used.
- **What you see.**

```bash
curl -s http://127.0.0.1:8084/health | jq '{ok, status, fallback}'
# {"ok": true, "status": "fallback",
#  "fallback": {"failed_generation": "visionGraph@<sha>", "reason": "<why it failed>",
#               "serving_generation": "visionGraph@<older sha>", "serving_dir": "/app/retained-bundles/<older sha>/data"}}
```

  `status` is `serving` in the normal case. `ok` stays `true`: the node answers, so the container
  health check and clients keep it, and the status says what it is answering from. `/loom/generation`
  carries the same `fallback` block (`null` normally); its phase stays `activated`, never `served`;
  `disk.matches_loaded` is `false`; the MCP manifest lists `generation-fallback` as degraded; and a
  `SERVING A FALLBACK GENERATION` warning is logged at start.
- **Reload tool.** `reload_published_generation` never records `served` for a fallback: a report
  with a `fallback` block does not prove the bundle. If a restart lands in a fallback it exits 1
  with `the service is serving a fallback generation ...` and the receipt stays `reload-pending`.
  If `data/` itself no longer verifies it refuses first, as always, and the receipt is untouched,
  so it still holds the last generation proved served: the exit status and `/health` are the signal.
- **Clearing it.** Nothing is stored. A successful `promote_generation` clears the cause: a newer
  commit, or the same generation rebuilt (which repairs an installed copy that no longer verifies).
  The nightly rebuilds when `data/` does not verify, so it does this unprompted; the reload tick then
  restarts the service onto it and `status` is `serving` again.
- **Left alone.** The service never writes to `data/` or to a retained bundle, so the failed bundle
  stays for inspection until a promotion replaces it. The semantic database is copied to
  `/run/loom/fallback/` before it is opened (redb mutates on open); with no writable runtime
  directory it is not opened and `/health` shows the semantic index unavailable.
- **Not covered.** A valid-looking but wrong bundle is not detected, and neither is a `data/` that
  verifies but fails to load for another reason; the reload tick's restart and probe cover the latter.

### Deploy on the Loom's host, and switch the nightly on

The running Loom is on the HP node (colocated with the model, `:8084` DNAT'd to the LAN; see
`docs/estate-network.md`). Nothing below is done by the repository; it is the owner's sequence, in
this order, run on HP (reach it from the machinelearn host shell: `ssh john@10.10.10.1`).

```bash
cd ~/githubs/loom
git fetch origin && git merge --ff-only origin/main      # after feat/nightly-promotion is merged

# 1. Build the three binaries (promote_vault_build lives in a different crate).
cargo build --release -p loom-facade --bin promote_generation --bin reload_published_generation
cargo build --release -p loom-vector-ruvector --bin promote_vault_build

# 2. The fallback mount's source must exist and be readable before the container is recreated,
#    or Docker creates it as root: install -d -m 0755 ~/.local/state/loom/bundles
#    Then rebuild the image so /health can report promoted_by, previous_generation and the start-up
#    fallback status, and recreate the container so the compose change (read-only mount of
#    ~/.local/state/loom/bundles and LOOM_RETAINED_BUNDLES_DIR) takes effect.
#    (See the build line at the top of deploy/compose.profile-a.yml; a rebuild restarts the
#    service onto the data directory as it is now.)

# 3. Dry-run the nightly once, by hand, as the service user. This also shows whether fetch and
#    release-download credentials exist in a systemd context. It builds, validates, installs and logs.
install -Dm644 deploy/loom-nightly-promotion.service ~/.config/systemd/user/loom-nightly-promotion.service
install -Dm644 deploy/loom-nightly-promotion.timer   ~/.config/systemd/user/loom-nightly-promotion.timer
systemctl --user daemon-reload                            # installs the units; enables nothing
systemctl --user start loom-nightly-promotion.service
journalctl --user -u loom-nightly-promotion.service -n 80 --no-pager
tail -n 1 data/promotions.jsonl | jq .
stat -c '%a %n' data data/.generation.json data/ontology.ttl      # expect 755 / 644 / 644

# 4. Let the existing reload tick restart the service onto it, then read the evidence.
systemctl --user start loom-generation-reload.service
cat ~/.local/state/loom/reload.json                        # expect "state": "served"
curl -s http://127.0.0.1:8084/health | jq '.generation | {id, promoted_at, promoted_by, previous_generation}'

# 5. Only then enable both timers. (Install the reload units first, as in
#    § When to enable the timer, if they are not installed yet.)
systemctl --user enable --now loom-nightly-promotion.timer loom-generation-reload.timer
loginctl enable-linger "$USER"
systemctl --user list-timers 'loom-*'
```

If the checkouts are not at `~/githubs/loom` and `~/githubs/visionGraph`,
set `LOOM_HOME`, `LOOM_DATA_DIR`, `LOOM_SOURCE_REPO` with `systemctl --user edit
loom-nightly-promotion.service` (`[Service]` then `Environment=...`). The container's user is
`nonroot` (uid 65532): `promote_generation` makes installed files 0644 and directories 0755 whatever
the build umask, so the container can read them. Each retained bundle directory is the full
`ontomark build` output, several hundred MB; `LOOM_KEEP_BUNDLES` (default 5) bounds the space.

To stop the nightly: `systemctl --user disable --now loom-nightly-promotion.timer`.
