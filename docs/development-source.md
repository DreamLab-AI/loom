# Reproducible development source

Loom deliberately consumes the `jjohare/ruvector` fork at commit
`677b2475409c50cb964be8a3b848da2952390535`, not upstream HEAD. Its sibling
layout is retained because RuVector core inherits its workspace dependencies.
The contract workflow checks out that exact revision and runs the same source
guard used locally before locked Cargo tests.

For a new workspace, clone Loom and the fork into adjacent `loom` and `ruvector`
directories, then detach the latter at that commit. Do not reset an existing
working checkout to achieve this: use a separate workspace when it contains work.

```sh
git clone https://github.com/jjohare/ruvector.git ruvector
git -C ruvector checkout --detach 677b2475409c50cb964be8a3b848da2952390535
cd loom
bash scripts/check-ruvector-source.sh
cargo test --workspace --locked
```

The guard checks the selected revision and modified/untracked consumer source,
including workspace manifest and lockfile. Changes in unrelated upstream packages
are not adopted. Updating the revision is a reviewed dependency change in both the
workflow and guard. CI has been authored; hosted execution and deployment are
separate evidence. No upstream ADR corpus is implicitly ratified by this pin.

Normal clones and linked Git worktrees produce the same release identity. The
build script follows `.git` files and `commondir`, resolves loose or packed refs,
and watches the branch ref as well as HEAD. A commit on the selected branch
therefore updates the baked receipt even when HEAD remains a symbolic ref.
Detached checkouts are supported; missing or malformed revisions remain
`unknown` and cannot qualify a release. Git is not required in the build image.

`loom-scaffold` consumes the versioned `ontomark-compile` index contracts with
default features disabled. Loom retains its own trust interpretation; the
compiler's optional reasoner does not enter the serving dependency graph. The
publishing CLI has a separate release tag and asset checksum in
`deploy/ontomark-release.env`.

## Provenance and review bindings (VisionFlow ADR-2024)

The shared Ontomark reader preserves source stamp scope and digest metadata.
Canonical units expose `content_attribution` only when the source supplies it and
`review_binding` only when the source bundle carries assessed freshness counts.
The legacy `generated: process:vault` fallback remains processing history; it is
not content authorship. A missing legacy review summary means unknown.

Generation descriptors retain an explicit `built_by` compiler name/version from
C3 markers. Readers never infer this from their own package version. Human review,
logical derivation and signed admission receipts retain their separate roles.
`is_human_verified` remains the historical verification query; callers needing
freshness must inspect the source review summary and individual stamp scopes.
A digest or a positive stamp count is not a signature or whole-page certification.
