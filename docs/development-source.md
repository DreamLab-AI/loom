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
