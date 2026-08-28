# metabook-py

Scaffolded from [exegia/corpora-py](https://github.com/exegia/corpora-py)'s CI/release pipeline: same branch model, GitHub Actions workflows, composite actions, and `make`-driven release automation. Only the app code is missing — this repo is plumbing-only until source is added.

## Status

This is a **scaffold**. There is no `src/` package yet, so `make ci` / `make pack` will fail until you:

1. Add your package source and point `[tool.hatch.build.targets.wheel]` in `pyproject.toml` at it (currently `src/metabook_py`).
2. Add tests under `tests/`.
3. Run `make setup` to install dependencies.

## Branching and release

See [`.github/WORKFLOW.md`](.github/WORKFLOW.md) for the full branch model, versioning rules, and workflow reference. Quick summary:

```
<type>/<slug> --PR--> dev --(daily/manual)--> next --cut--> release/vX.Y.Z --draft PR--> main
                 (deleted on merge)         (preview)                       (deleted on release)
```

## Common commands

```bash
make help              # list all targets
make setup              # install dependencies (uv sync)
make ci                 # everything CI runs on a PR (lint + test)
make pack                # build the publishable wheel
make rulesets-diff       # rulesets GitHub currently has
make rulesets-apply      # push .github/rulesets/*.json
```

## Bootstrap

There are no `dev` / `next` branches until the first wrapup. Run the **Release** workflow manually (`Actions → Release → Run workflow`), or locally:

```bash
make bootstrap-lanes
```
