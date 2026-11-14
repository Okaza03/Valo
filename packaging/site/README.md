# Website

The site at <https://okaza03.github.io/Valo/>: React, Tailwind, [shadcn/ui](https://ui.shadcn.com) components and [Magic UI](https://magicui.design) animations (added with the shadcn CLI, so they live in `src/components/ui/`).

```
npm ci
npm run dev      # http://localhost:5173
npm run build    # dist/, which packaging/flatpak/publish-repo.sh publishes
```

- `src/blocks/`: the page's sections.
- `src/downloads.ts`: every file a release ships, and which one suits the visitor's system. Keep the names in step with the release workflow.
- `index.html` holds `@BASE@`, `@DL@`, `@TAG@` and `@VERSION@`, which `publish-repo.sh` fills in. Left as they are (`npm run dev`), the page asks GitHub for the latest release.

Merging a change here into main deploys it through `.github/workflows/pages.yml`, without a release.
