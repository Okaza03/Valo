// Where the download links point. publish-repo.sh fills in the JSON in
// index.html; a local preview (placeholders left as they are) uses the latest
// release instead, so every link still works.

const REPO = "Okaza03/Valo"
export const GITHUB = `https://github.com/${REPO}`
export const RELEASES = `${GITHUB}/releases`

type Raw = { base: string; dl: string; tag: string; version: string }

function read(): Raw | null {
  try {
    const el = document.getElementById("valo-release")
    const raw = JSON.parse(el?.textContent ?? "") as Raw
    return raw.tag.includes("@") ? null : raw
  } catch {
    return null
  }
}

const raw = read()

export const release = {
  /** false in a local preview: links go to the latest release by name */
  pinned: raw !== null,
  base: raw?.base ?? "https://okaza03.github.io/Valo",
  tag: raw?.tag ?? "",
  version: raw?.version ?? "",
  dl: raw?.dl ?? `${RELEASES}/latest/download`,
}

// File names on the release, the same ones the release workflow uploads.
// Without a pinned tag (preview) they can't be known exactly, so the GitHub
// API fills the version in once it answers; until then names use "latest".
export function fileName(pattern: string, tag: string, version: string) {
  return pattern.replaceAll("{tag}", tag).replaceAll("{version}", version)
}

export type Asset = { size: number; downloads: number }

/** Sizes and download counts of the release's files, by file name. */
export async function fetchAssets(): Promise<{
  tag: string
  published: string
  assets: Map<string, Asset>
} | null> {
  const url = release.pinned
    ? `https://api.github.com/repos/${REPO}/releases/tags/${release.tag}`
    : `https://api.github.com/repos/${REPO}/releases/latest`
  try {
    const res = await fetch(url, { headers: { Accept: "application/vnd.github+json" } })
    if (!res.ok) return null
    const json = (await res.json()) as {
      tag_name: string
      published_at: string
      assets: { name: string; size: number; download_count: number }[]
    }
    return {
      tag: json.tag_name,
      published: json.published_at,
      assets: new Map(json.assets.map((a) => [a.name, { size: a.size, downloads: a.download_count }])),
    }
  } catch {
    return null
  }
}

export function formatSize(bytes: number) {
  const mb = bytes / (1024 * 1024)
  return mb >= 10 ? `${Math.round(mb)} MB` : `${mb.toFixed(1)} MB`
}
