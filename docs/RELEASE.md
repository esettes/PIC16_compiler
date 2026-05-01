<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

Release Guide

This project maintains release notes under `docs/releases/` and a top-level `docs/CHANGELOG.md`.

To create a release locally:

1. Bump the version in `Cargo.toml` (e.g., `version = "0.1.1"`).
2. Commit your changes:

```bash
git add -A
git commit -m "chore(release): vX.Y.Z"
```

3. Tag the release:

```bash
git tag -a vX.Y.Z -m "pic16cc vX.Y.Z"
git push origin --tags
```

4. Draft a GitHub release and paste the contents of `docs/releases/vX.Y.Z.md` into the release notes.

CI/Validation

- Before tagging, ensure tests and linters are clean:

```bash
cargo test --all
cargo clippy --all-targets -- -D warnings
```

- Optionally build the release binary and attach it to the GitHub release:

```bash
cargo build --release
```

If you want, I can prepare the commit and tag for v0.1.0 (or another version) and produce a release draft locally; tell me which tag name you want and whether to include compiled artifacts.
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
