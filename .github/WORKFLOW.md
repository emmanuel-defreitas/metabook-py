# Branching and release

Two long-lived integration lanes (`dev`, `next`) plus `main` and the tags.
Release branches are temporary and versioned.

```
<type>/<slug> ──PR──> dev ──(daily/manual)──> next ──cut──> release/vX.Y.Z ──draft PR──> main
                 (deleted on merge)         (preview)                       (deleted on release)
```

Adapted from [exegia/corpora-py](https://github.com/exegia/corpora-py). This
repo is currently a scaffold only: merging a release tags `vX.Y.Z`, which
triggers `publish.yml` to ship the wheel to PyPI (once a `pypi` trusted
publisher is configured for this repo). There is no deploy job.

## Feature branches

Named `<type>/<slug>` — `feat`, `fix`, `chore`, `docs`, `ci`, `refactor`,
`test`, `perf`, `build`, `style`, `revert`. (Git forbids `:` in a ref name, so
the conventional-commit form lives in the **PR title**: `feat: add parser`.)

Branch off `dev` and open a PR back into it. While the PR is a draft only the
guard runs; marking it **ready for review** starts the tests and the AI review
(if `CLAUDE_CODE_OAUTH_TOKEN` is configured), which then re-run on every push.
`guard` and `check` are required, so a red one cannot land.

**Stacked PRs** (a feature branch based on another feature branch, not `dev`
directly) are supported: the guard validates a `<type>/<slug>` base the same
way it validates the head, so `feat/b → feat/a → dev` passes as long as both
branches and the title follow the convention. Merge bottom-up; each merge
rebases the remaining stack onto `dev`.

When it merges the branch deletes itself (repository setting) and `dev` moves
forward. Nothing is versioned yet.

Dependabot is the one exception: it opens `dependabot/<ecosystem>/<dep>-<ver>`
with a `Bump X from A to B` title, and neither is renameable. `make pr-guard`
waves those through whichever branch they target.

## `dev` and `next`

`dev` is the working branch. Features land here all day.

`next` is staging. A scheduled workflow (22:00 UTC daily) and a manual
**Promote to next** action open a PR from `dev` into `next` when `dev` is
ahead. That PR auto-merges once `guard` and `check` pass. Every push to
`next` also runs the os × Python matrix.

## Versioning

The promote job classifies the bump from line-count churn
(`git diff --shortstat origin/next...origin/dev`, insertions + deletions):

| Churn | Makefile bump | Semver | Your label |
|-------|---------------|--------|------------|
| `< 100` | `patch` | `0.0.+1` | minor |
| `100–999` | `minor` | `0.+1.0` | major |
| `≥ 1000` | `major` | `+1.0.0` | breaking |

`workflow_dispatch` can override with `major` / `minor` / `patch`. The chosen
version is stored on the promote PR as `<!-- release: vX.Y.Z -->` so the cut
still knows it after `dev` and `next` are equal.

The version is written into `pyproject.toml` plus `uv.lock`. Version lives
only on `release/v*` until `main` is merged back.

## Release branches

Named `release/vX.Y.Z`, and always carry that version in `pyproject.toml`.
The guard rejects a PR into `main` where the root and the branch name
disagree.

A push to `next` cuts (or refreshes) the branch from `next` plus a
`chore(release): open vX.Y.Z` commit. Exactly one is in flight at a time: if a
draft PR into `main` is already open, later promotions fast-forward that same
branch and **keep its version**.

Last-minute fixes can still PR `<type>/<slug>` directly into that in-flight
`release/v*` (same guard/check as `dev`). A push refreshes the draft into
`main`.

Its ruleset deliberately omits `creation` and `deletion` rules — `make
cut-release` has to create it and `make delete-branch` has to remove it after
the release. The automation App is on the bypass list so it can push a
refresh without opening a PR (once one is configured — see Secrets below).

## `main`

No direct pushes; PRs only from `release/vX.Y.Z`. Merging one creates the
`vX.Y.Z` tag and GitHub Release, deletes the release branch, then opens PRs
that merge `main` back into `next` and `dev` and deletes leftover remote
feature / `release/v*` heads.

It does **not** cut the next release branch. That waits for the next promote.

Everything downstream hangs off that tag: `publish.yml` sends the wheel to
PyPI.

## Workflows

| File                | Trigger                         | Does                                                |
| ------------------- | -------------------------------- | --------------------------------------------------- |
| `pr.yml`            | PR opened / ready / pushed       | `guard`, `check`, `package` (into main), `review` (into `dev`) |
| `promote.yml`       | 22:00 UTC daily / manual         | bootstrap lanes, open `dev` → `next` PR, auto-merge |
| `next.yml`          | push to `next`                   | cut/refresh `release/v*`                            |
| `pr-merged.yml`     | push to `release/v*`             | upsert the draft release PR into `main`             |
| `release.yml`       | PR merged into `main`            | tag, sync lanes, cleanup                            |
| `matrix.yml`        | push to `next` / `release/v*`, weekly | os × Python coverage                          |
| `publish.yml`       | push of a `vX.Y.Z` tag           | builds and publishes the wheel to PyPI              |
| `automerge.yml`     | Dependabot PR                    | enables auto-merge                                  |

Every step in the first five is a `make` target, so anything CI does can be
reproduced locally.

### Merge methods differ by level

Feature PRs into `dev` (or an in-flight `release/v*`) are **squashed** — that
is this repo's convention and it keeps each feature one commit. PRs into
`next` and the release PR into `main` are a **merge**, and the rulesets
enforce that. Squashing the release PR would collapse the whole release into
a single commit, and `gh release --generate-notes` would have nothing to list.

### The tag must not be created by `GITHUB_TOKEN`

`make tag-release` should run with the automation App's token, and that is
load-bearing rather than incidental. Events raised by `GITHUB_TOKEN` do not
start new workflow runs, so a tag pushed with it would leave `publish.yml`
sitting there, never firing, with nothing to indicate the release had not
shipped.

The same rule is why `pr-merged.yml`, `promote.yml` and `next.yml` run as the
App: a PR opened by `GITHUB_TOKEN` cannot trigger further workflows, and
`guard` and `check` are *required* checks — those PRs would never be
mergeable.

### Apply the rulesets after the first cut lands on `main`

`.github/rulesets/*.json` is inert until `make rulesets-apply`. Each file's
`bypass_actors` currently only lists the repository-admin role (`actor_id:
5`, `RepositoryRole`) — add your automation App's Integration id to each
file once one is set up, the same way `corpora-py` does, or the App won't be
able to push refreshes without opening a PR for itself.

```bash
make rulesets-diff     # what GitHub has now
make rulesets-apply    # push all five files
```

`make rulesets-apply` matches by `.name`, so a file must keep the name of the
ruleset already on GitHub (`Protect main branch`, `Protect dev branch`,
`Protect next branch`, `Protect release branches`, `Publishing`) or a second
one is created alongside it.

Enable **Allow auto-merge** on the repository. Promote and post-release sync
PRs use `gh pr merge --auto`.

## Bootstrap and manual operations

There are no `dev` / `next` branches until the first wrapup. Run the
**Release** workflow manually (`Actions → Release → Run workflow`) — the
release job skips and wrapup creates the lanes. Locally:

```bash
make bootstrap-lanes
```

`dev` is created from the newest `release/v*` if one exists, otherwise `main`.
`next` is created from `main`. Then run **Promote to next** (or wait until
22:00 UTC) once there is work on `dev`.

Other useful targets:

```bash
make ci                            # what CI runs on a PR
make pack                          # build the publishable wheel
make pkg-version                   # the version in pyproject.toml
make churn-info FROM=origin/next TO=origin/dev
make next-version BUMP=patch       # what the next tag would be called
make release-notes RANGE=origin/main..HEAD
make cleanup-local                 # prune local feature / release branches
make rulesets-diff                 # rulesets GitHub actually has
make rulesets-apply                # push .github/rulesets/*.json
```

`make tag-release` is idempotent — a tag already released is skipped, not an
error.

Everything is parameterised on `TRUNK`, so a one-off against a different trunk
is `make pr-guard TRUNK=some-branch ...`.

Scheduled workflows are read from the **default branch**. `promote.yml` will
not fire on a cron until this file has shipped to `main`.

## Secrets

| Name                                               | Where            | Used by                     |
| --------------------------------------------------- | ---------------- | --------------------------- |
| `AUTOMATION_APP_ID` / `AUTOMATION_APP_PRIVATE_KEY`   | repo or org      | opening PRs, branches, tags |
| `CLAUDE_CODE_OAUTH_TOKEN`                            | repo or org      | the AI review (optional)    |

Neither is set up yet on this scaffold. Without the automation App secrets,
`promote.yml`, `next.yml`, `pr-merged.yml` and the `wrapup` job in
`release.yml` will fail at their first step — a PR opened with `GITHUB_TOKEN`
cannot trigger further workflows, so the release PR's own checks would never
start. Set these up (a small GitHub App with `contents: write` +
`pull_requests: write`, installed on this repo) before relying on the
automated promote/release flow.

PyPI needs no secret once set up — `publish.yml` uses OIDC trusted publishing
against the `pypi` environment. **That binding names the workflow file**, so
renaming `publish.yml` breaks it until the publisher is re-pointed on PyPI.

Without `CLAUDE_CODE_OAUTH_TOKEN` the review job skips with a note in the job
summary rather than failing.
