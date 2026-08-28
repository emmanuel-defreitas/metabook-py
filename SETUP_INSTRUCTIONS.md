# Pushing this to emmanuel-defreitas/metabook-py

The GitHub connector in this session could read the repo but not write to it (403 on every
write attempt, even after permission changes), so here's everything ready to push from your own
terminal instead.

## 1. Copy these files into place

Unzip the bundle, then copy the `metabook-py/` folder's contents into a local clone of the (still
empty) repo:

```bash
git clone https://github.com/emmanuel-defreitas/metabook-py.git
cp -r bundle/metabook-py/. metabook-py/
cd metabook-py
```

(If you'd rather not clone first, `cd` into the unzipped `metabook-py/` folder directly and run
`git init` there instead — see step 2.)

## 2. Init, commit, push

```bash
# If you copied into a fresh clone (recommended):
git add -A
git commit -m "chore: scaffold CI/release pipeline from exegia/corpora-py"
git push origin main

# If you're initializing in place instead:
git init -b main
git remote add origin https://github.com/emmanuel-defreitas/metabook-py.git
git add -A
git commit -m "chore: scaffold CI/release pipeline from exegia/corpora-py"
git push -u origin main
```

## 3. What's included

- `.github/workflows/` — 8 workflows: `pr.yml` (guard/check/package/review), `promote.yml`,
  `next.yml`, `pr-merged.yml`, `release.yml`, `matrix.yml`, `publish.yml`, `automerge.yml`.
  Docker, Vercel-demo, and native-sidecar workflows were intentionally dropped (scaffolding only).
- `.github/actions/` — 3 composite actions: `setup`, `build-dist`, `publish-pypi`.
- `.github/rulesets/` — 5 branch/tag protection rulesets (`dev`, `main`, `next`, `release`, `tags`).
  Each currently only bypasses the repository-admin role; add an automation App's Integration id
  once you set one up (see `.github/WORKFLOW.md` → Secrets).
- `.github/WORKFLOW.md` — full branch model and workflow reference.
- `makefile` + `bin/` — trimmed to the CI/release-relevant targets (no Docker/Vercel/bun).
- `pyproject.toml`, `.python-version`, `LICENSE`, `.gitignore`, `README.md`.

## 4. After pushing

1. Run **Actions → Release → Run workflow** once to bootstrap the `dev` and `next` branches
   (there's nothing to release yet, but this creates the lanes).
2. Set up the `AUTOMATION_APP_ID` / `AUTOMATION_APP_PRIVATE_KEY` secrets (a small GitHub App with
   `contents: write` + `pull_requests: write`) before relying on `promote.yml` / `next.yml` /
   `pr-merged.yml` — without it those fail immediately (see `.github/WORKFLOW.md` → Secrets).
3. `make rulesets-apply` once the App exists, to push the branch-protection rulesets live.
4. Add real source under `src/metabook_py/` and tests under `tests/` — `make ci` / `make pack`
   won't succeed until then.
