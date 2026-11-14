import { useEffect, useState } from "react"
import { fetchAssets, fileName, release, RELEASES, type Asset } from "./release"

export type PlatformId = "windows" | "linux" | "android"

export type FileEntry = {
  id: string
  /** release file name; {tag} is v0.9.14, {version} 0.9.14 */
  pattern: string
  title: string
  detail: string
  arch?: "x64" | "ARM64"
  badges?: string[]
}

export type Platform = {
  id: PlatformId
  name: string
  blurb: string
  files: FileEntry[]
}

export const platforms: Platform[] = [
  {
    id: "windows",
    name: "Windows",
    blurb: "Windows 10 and 11. The installer is for your user only, so it doesn't need an admin password.",
    files: [
      {
        id: "win-x64",
        pattern: "valo-{tag}-x86_64-setup.exe",
        title: "Installer",
        detail: "Windows 10 and 11, 64-bit",
        arch: "x64",
        badges: ["Updates itself"],
      },
      {
        id: "win-arm",
        pattern: "valo-{tag}-aarch64-setup.exe",
        title: "Installer for ARM64",
        detail: "Windows 11 on ARM, like Snapdragon laptops",
        arch: "ARM64",
        badges: ["Updates itself"],
      },
      {
        id: "win-zip",
        pattern: "valo-{version}-windows-x86_64.zip",
        title: "Portable zip",
        detail: "No install: unzip and run valo.exe",
        arch: "x64",
      },
      {
        id: "win-zip-arm",
        pattern: "valo-{version}-windows-aarch64.zip",
        title: "Portable zip for ARM64",
        detail: "No install, for Windows on ARM",
        arch: "ARM64",
      },
    ],
  },
  {
    id: "linux",
    name: "Linux",
    blurb: "Any distro. The Flatpak and the AppImage update themselves.",
    files: [
      {
        id: "appimage",
        pattern: "valo-{tag}-x86_64.AppImage",
        title: "AppImage",
        detail: "Any distro: make it executable and run it",
        arch: "x64",
        badges: ["Updates itself"],
      },
      {
        id: "flatpak",
        pattern: "valo-{tag}-x86_64.flatpak",
        title: "Flatpak bundle",
        detail: "One file for flatpak install --user, same updates as the repository",
        arch: "x64",
        badges: ["Updates itself"],
      },
      {
        id: "deb",
        pattern: "valo_{version}-1_amd64.deb",
        title: ".deb package",
        detail: "Debian, Ubuntu, Mint",
        arch: "x64",
      },
      {
        id: "tar",
        pattern: "valo-{version}-linux-x86_64.tar.gz",
        title: "Tarball",
        detail: "To install by hand",
        arch: "x64",
      },
    ],
  },
  {
    id: "android",
    name: "Android",
    blurb: "Android TV, Google TV, phones and tablets. Both apps update themselves.",
    files: [
      {
        id: "tv",
        pattern: "valo-{tag}-androidtv.apk",
        title: "Android TV and Google TV",
        detail: "Made for the remote, plays with the TV's own decoders",
        badges: ["Updates itself"],
      },
      {
        id: "phone",
        pattern: "valo-{tag}-android.apk",
        title: "Android phones and tablets",
        detail: "Made for touch, with a Cast button for Chromecast",
        badges: ["Updates itself", "Cast"],
      },
    ],
  },
]

export const allFiles = platforms.flatMap((p) => p.files)

// ---- which file suits this visitor

export type Detected =
  | { kind: "file"; platform: PlatformId; fileId: string; label: string }
  | { kind: "flatpak"; label: string }
  | { kind: "unsupported"; label: string }

async function isArm(): Promise<boolean> {
  type UAData = { getHighEntropyValues(h: string[]): Promise<{ architecture?: string }> }
  const data = (navigator as Navigator & { userAgentData?: UAData }).userAgentData
  if (data?.getHighEntropyValues) {
    try {
      const v = await data.getHighEntropyValues(["architecture"])
      return v.architecture === "arm"
    } catch {
      /* fall through */
    }
  }
  return /arm64|aarch64/i.test(navigator.userAgent)
}

export function useDetected(): Detected | null {
  const [d, setD] = useState<Detected | null>(null)
  useEffect(() => {
    const ua = navigator.userAgent
    let cancelled = false
    ;(async () => {
      let r: Detected
      if (/Android/i.test(ua)) {
        const tv = /\b(TV|AFT|BRAVIA|SMART-TV|GoogleTV)\b/i.test(ua)
        r = tv
          ? { kind: "file", platform: "android", fileId: "tv", label: "Android TV" }
          : { kind: "file", platform: "android", fileId: "phone", label: "Android" }
      } else if (/iPhone|iPad|iPod/i.test(ua)) {
        r = { kind: "unsupported", label: "iOS" }
      } else if (/Windows/i.test(ua)) {
        const arm = await isArm()
        r = { kind: "file", platform: "windows", fileId: arm ? "win-arm" : "win-x64", label: arm ? "Windows on ARM" : "Windows" }
      } else if (/Mac OS X|Macintosh/i.test(ua)) {
        r = { kind: "unsupported", label: "macOS" }
      } else if (/Linux|X11|CrOS/i.test(ua)) {
        r = { kind: "flatpak", label: "Linux" }
      } else {
        r = { kind: "unsupported", label: "your system" }
      }
      if (!cancelled) setD(r)
    })()
    return () => {
      cancelled = true
    }
  }, [])
  return d
}

// ---- the release the links point at

export type ReleaseInfo = {
  tag: string
  version: string
  published?: string
  assets: Map<string, Asset>
  /** a link for a file, or the release page while the version isn't known */
  href(f: FileEntry): string
  name(f: FileEntry): string
  totalDownloads: number
}

export function useRelease(): ReleaseInfo {
  const [state, setState] = useState<{ tag: string; published?: string; assets: Map<string, Asset> }>({
    tag: release.tag,
    assets: new Map(),
  })
  useEffect(() => {
    fetchAssets().then((r) => {
      if (r) setState({ tag: release.pinned ? release.tag : r.tag, published: r.published, assets: r.assets })
    })
  }, [])
  const tag = state.tag
  const version = tag.replace(/^v/, "")
  const dl = release.pinned ? release.dl : `${RELEASES}/download/${tag}`
  let total = 0
  for (const a of state.assets.values()) total += a.downloads
  return {
    tag,
    version,
    published: state.published,
    assets: state.assets,
    name: (f) => fileName(f.pattern, tag, version),
    href: (f) => (tag ? `${dl}/${fileName(f.pattern, tag, version)}` : `${RELEASES}/latest`),
    totalDownloads: total,
  }
}
