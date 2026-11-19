# Changesets

Every pull request adds a changeset: a small Markdown file here saying what changed for users and how big a change it is. [changeset-bot](https://github.com/apps/changeset-bot) checks each PR for one.

Make one with `npx @changesets/cli` (pick `valo` and the bump), or write it by hand as `.changeset/<some-name>.md`:

```md
---
"valo": minor
---

Hardware video decoding, with a setting to turn it off.
```

- `patch`: fixes and small changes.
- `minor`: new features.
- `major`: a change that breaks something for users or server admins.

A change users won't notice (CI, docs, refactoring) gets an empty changeset, so the bot knows it was thought about:

```md
---
---

CI: fetch newer Vulkan headers.
```

## At release time

In the `release-X.Y.Z` branch, run `npx @changesets/cli version`. It adds up the changesets into the next version, bumps `package.json`, writes `CHANGELOG.md` and deletes the changesets. Then set the same version in `Cargo.toml` and write `notes.md` from the new `CHANGELOG.md` entry (see [Releasing](../README.md#releasing)).

`package.json` exists only for this; the app's version is the one in `Cargo.toml`.
