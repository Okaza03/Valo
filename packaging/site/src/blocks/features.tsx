import { CheckIcon, LoaderCircleIcon, SearchIcon } from "lucide-react"
import { AnimatePresence, motion, useInView, useReducedMotion } from "motion/react"
import { useEffect, useRef, useState, type ReactNode } from "react"
import { PlatformIcon } from "@/components/brand-icons"
import { Poster } from "@/components/poster"
import { SectionHeading } from "@/components/section-heading"
import { BlurFade } from "@/components/ui/blur-fade"
import { Marquee } from "@/components/ui/marquee"
import { platforms } from "@/downloads"
import { cn } from "@/lib/utils"

/** Steps 0..n-1 every `ms` while on screen. */
function useTicker(n: number, ms: number) {
  const ref = useRef<HTMLDivElement>(null)
  const seen = useInView(ref, { amount: 0.4 })
  const reduce = useReducedMotion()
  const [i, setI] = useState(0)
  useEffect(() => {
    if (!seen || reduce) return
    const t = setInterval(() => setI((x) => (x + 1) % n), ms)
    return () => clearInterval(t)
  }, [seen, reduce, n, ms])
  return [ref, i] as const
}

function Cell({ title, body, children, className, delay = 0 }: { title: string; body: string; children: ReactNode; className?: string; delay?: number }) {
  return (
    <BlurFade inView delay={delay} className={cn("h-full", className)}>
      <div className="group relative flex h-full flex-col overflow-hidden rounded-3xl border border-white/[0.07] bg-gradient-to-b from-white/[0.04] to-white/[0.01] transition-colors duration-500 hover:border-white/15">
        <div className="relative min-h-52 flex-1 overflow-hidden">{children}</div>
        <div className="relative p-6 pt-4 sm:p-7 sm:pt-4">
          <h3 className="mb-1.5 text-lg font-semibold tracking-tight">{title}</h3>
          <p className="text-[15px] leading-relaxed text-muted-foreground">{body}</p>
        </div>
      </div>
    </BlurFade>
  )
}

// ---- previews

const queries = ["Sci-fi", "Christopher", "Season 2", "Documentary"]

function LibraryPreview() {
  const [ref, i] = useTicker(queries.length, 2600)
  return (
    <div ref={ref} className="absolute inset-0 flex flex-col gap-4 pt-7">
      <div className="mx-7 flex h-10 items-center gap-2.5 rounded-xl border border-white/10 bg-black/40 px-3.5 text-sm">
        <SearchIcon className="size-4 text-muted-foreground" />
        <AnimatePresence mode="wait">
          <motion.span key={i} initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -6 }} transition={{ duration: 0.25 }}>
            {queries[i]}
          </motion.span>
        </AnimatePresence>
        <span className="h-4 w-px animate-pulse bg-primary" />
        <span className="ml-auto hidden font-mono text-[11px] text-muted-foreground sm:inline">titles · genres · directors</span>
      </div>
      <div className="[mask-image:linear-gradient(90deg,transparent,#000_8%,#000_92%,transparent)]">
        <Marquee pauseOnHover className="[--duration:38s] [--gap:0.75rem]" repeat={3}>
          {Array.from({ length: 8 }, (_, k) => (
            <Poster key={k} i={k + i * 2} className="w-24 sm:w-28" />
          ))}
        </Marquee>
      </div>
    </div>
  )
}

function ContinuePreview() {
  const [ref, i] = useTicker(6, 1000)
  const left = 5 - i
  return (
    <div ref={ref} className="absolute inset-0 grid place-items-center p-7">
      <div className="relative w-full max-w-xs overflow-hidden rounded-xl border border-white/10" style={{ background: "linear-gradient(140deg,#16473c,#3da88a 60%,#a8963d)" }}>
        <div className="aspect-video" />
        <div className="absolute inset-0 bg-gradient-to-t from-black/85 via-black/20 to-transparent" />
        <div className="absolute inset-x-4 bottom-4">
          <p className="text-[11px] text-white/60">Next episode</p>
          <p className="mb-3 text-sm font-semibold text-white">S2 · E4</p>
          <div className="flex items-center gap-3">
            <div className="relative size-9">
              <svg viewBox="0 0 36 36" className="size-9 -rotate-90">
                <circle cx="18" cy="18" r="15" fill="none" stroke="rgba(255,255,255,.2)" strokeWidth="3" />
                <motion.circle
                  cx="18" cy="18" r="15" fill="none" stroke="#fff" strokeWidth="3" strokeLinecap="round"
                  strokeDasharray={94.2}
                  animate={{ strokeDashoffset: 94.2 * (left / 5) }}
                  transition={{ duration: 0.9, ease: "linear" }}
                />
              </svg>
              <span className="absolute inset-0 grid place-items-center text-xs font-semibold text-white tabular-nums">{left}</span>
            </div>
            <div className="h-1 flex-1 rounded-full bg-white/25">
              <div className="h-full w-[96%] rounded-full bg-[#4a7cf7]" />
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

function SubtitlePreview() {
  const [ref, i] = useTicker(4, 1500)
  const offsets = ["+0.8 s", "+0.4 s", "0.0 s", "0.0 s"]
  const synced = i >= 2
  return (
    <div ref={ref} className="absolute inset-0 grid place-items-center p-7">
      <div className="relative w-full max-w-xs overflow-hidden rounded-xl border border-white/10 bg-[linear-gradient(140deg,#1d2a4f,#3d5ea8_55%,#8a3da8)]">
        <div className="aspect-video" />
        <div className="absolute top-3 right-3 rounded-md bg-black/60 px-2 py-1 font-mono text-[11px] text-white tabular-nums backdrop-blur">
          Delay {offsets[i]}
        </div>
        <motion.p
          className="absolute inset-x-4 bottom-4 text-center text-[13px] font-medium text-white [text-shadow:0_1px_4px_#000]"
          animate={{ opacity: synced ? 1 : 0.45, y: synced ? 0 : 4 }}
        >
          <span className="rounded bg-black/50 px-1.5 py-0.5">We should have left an hour ago.</span>
        </motion.p>
      </div>
    </div>
  )
}

const themes = [
  { name: "System", bg: "linear-gradient(90deg,#121317 50%,#f2f2f4 50%)", fg: "#ececee", card: "#2a2b31" },
  { name: "Dark", bg: "#121317", fg: "#ececee", card: "#24252b" },
  { name: "Light", bg: "#f2f2f4", fg: "#18181b", card: "#dcdce1" },
  { name: "Black", bg: "#000000", fg: "#ececee", card: "#16161a" },
  { name: "Dim", bg: "#2a2b30", fg: "#ececee", card: "#3a3b42" },
  { name: "Midnight", bg: "#0c1430", fg: "#e6ebff", card: "#1a2550" },
]

function LookPreview() {
  const [ref, i] = useTicker(themes.length, 1700)
  const t = themes[i]
  return (
    <div ref={ref} className="absolute inset-0 flex flex-col items-center justify-center gap-4 p-7">
      <motion.div className="w-full max-w-xs rounded-xl border border-white/10 p-3" animate={{ background: t.bg }} transition={{ duration: 0.6 }}>
        <motion.div className="mb-2.5 h-2 w-16 rounded-full" animate={{ background: t.fg }} />
        <div className="grid grid-cols-4 gap-2">
          {[0, 1, 2, 3].map((k) => (
            <motion.div key={k} className="aspect-[2/3] rounded-md" animate={{ background: t.card }} transition={{ duration: 0.6, delay: k * 0.05 }} />
          ))}
        </div>
      </motion.div>
      <div className="flex flex-wrap justify-center gap-1.5">
        {themes.map((th, k) => (
          <span
            key={th.name}
            className={cn(
              "rounded-full border px-2.5 py-0.5 font-mono text-[11px] transition-colors duration-300",
              k === i ? "border-primary/60 bg-primary/15 text-foreground" : "border-white/10 text-muted-foreground",
            )}
          >
            {th.name}
          </span>
        ))}
      </div>
    </div>
  )
}

const checks = ["Reach the server", "Sign-in", "Library API · GET /api/library", "Streaming · Range requests", "Artwork", "Progress sync"]

function CheckPreview() {
  const [ref, i] = useTicker(checks.length + 3, 650)
  return (
    <div ref={ref} className="absolute inset-0 flex items-center justify-center p-7">
      <ul className="w-full max-w-xs space-y-2 rounded-xl border border-white/10 bg-black/40 p-4 font-mono text-[12px]">
        {checks.map((c, k) => (
          <li key={c} className="flex items-center gap-2.5">
            <span className="grid size-4 place-items-center">
              {k < i ? (
                <motion.span initial={{ scale: 0 }} animate={{ scale: 1 }} className="grid size-4 place-items-center rounded-full bg-emerald-500/20 text-emerald-400">
                  <CheckIcon className="size-3" />
                </motion.span>
              ) : k === i ? (
                <LoaderCircleIcon className="size-3.5 animate-spin text-muted-foreground" />
              ) : (
                <span className="size-1.5 rounded-full bg-white/20" />
              )}
            </span>
            <span className={k < i ? "text-foreground/90" : "text-muted-foreground/60"}>{c}</span>
          </li>
        ))}
      </ul>
    </div>
  )
}

function FastPreview() {
  // a frame-time graph: every frame on the beat, so no tearing
  return (
    <div className="absolute inset-0 flex flex-col justify-center gap-5 px-7 pt-7">
      <div className="flex flex-wrap gap-2">
        {["Rust", "FFmpeg", "GPU", "dav1d"].map((t) => (
          <span key={t} className="rounded-md border border-white/10 bg-white/[0.03] px-2.5 py-1 font-mono text-xs">
            {t}
          </span>
        ))}
      </div>
      <div className="relative h-24 overflow-hidden rounded-xl border border-white/10 bg-black/40">
        <div className="absolute inset-x-0 top-1/2 border-t border-dashed border-primary/40" />
        <span className="absolute top-2 right-3 font-mono text-[10px] text-primary/80">in step with your screen</span>
        <div className="absolute inset-y-0 left-0 flex w-[200%] animate-[drift-left_10s_linear_infinite]">
          {/* two identical halves, so the -50% loop is seamless */}
          {[0, 1].map((half) => (
            <div key={half} className="flex w-1/2 items-end gap-1 px-0.5 pb-2">
              {Array.from({ length: 72 }, (_, k) => (
                <span key={k} className="flex-1 rounded-sm bg-[#4a7cf7]/70" style={{ height: `${46 + ((k * 7) % 5)}%` }} />
              ))}
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}

// ---- Infuse-style spec strip: where it runs, where it plays from

export function Specs() {
  const rows: { label: string; items: ReactNode[] }[] = [
    {
      label: "Runs on",
      items: platforms.map((p) => (
        <span key={p.id} className="inline-flex items-center gap-2">
          <PlatformIcon id={p.id} className="size-4 opacity-80" />
          {p.id === "android" ? "Android TV & phones" : p.name}
        </span>
      )),
    },
    { label: "Plays from", items: ["Valo's bundled server", "Your own app's API", "nginx, Apache & Caddy listings"].map((t) => <span key={t}>{t}</span>) },
    { label: "Subtitles", items: ["From your server", "Subtitle addons", "Timing you can shift"].map((t) => <span key={t}>{t}</span>) },
  ]
  return (
    <div className="mx-auto max-w-6xl px-4 sm:px-6">
      <div className="divide-y divide-white/[0.07] border-y border-white/[0.07]">
        {rows.map((r, k) => (
          <BlurFade key={r.label} inView delay={k * 0.06}>
            <div className="flex flex-col gap-2 py-5 sm:flex-row sm:items-center sm:gap-8">
              <span className="w-32 shrink-0 font-mono text-xs tracking-[0.14em] text-muted-foreground uppercase">{r.label}</span>
              <div className="flex flex-wrap gap-x-7 gap-y-2 text-[15px] text-foreground/85">{r.items}</div>
            </div>
          </BlurFade>
        ))}
      </div>
    </div>
  )
}

export function Features() {
  return (
    <section id="features" className="relative pt-28 pb-12 sm:pt-36 sm:pb-16">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <SectionHeading index="01" kicker="Features" title={<>A streaming app, <em className="text-primary">for the files you own.</em></>}>
          Valo doesn't host anything. It plays straight from your own server, on your own network.
        </SectionHeading>

        <div className="grid auto-rows-[minmax(0,auto)] gap-4 md:grid-cols-6">
          <Cell
            className="md:col-span-4"
            title="A real library"
            body="A featured banner and rows for Continue watching, My list, Recently added and every genre. Grids sort and filter, and search finds titles, genres and directors."
          >
            <LibraryPreview />
          </Cell>
          <Cell
            className="md:col-span-2"
            delay={0.08}
            title="Picks up where you left off"
            body="Plays the next episode for you, and syncs your progress so another device carries on from the same spot."
          >
            <ContinuePreview />
          </Cell>
          <Cell
            className="md:col-span-2"
            title="Subtitles that fit"
            body="From the server or its subtitle addons. Your language is remembered, and a file that's a little off can be shifted while you watch."
          >
            <SubtitlePreview />
          </Cell>
          <Cell
            className="md:col-span-2"
            delay={0.08}
            title="Your look"
            body="Six color themes, including black for OLED screens and following your desktop, and seven styles that change how pages and the player are laid out."
          >
            <LookPreview />
          </Cell>
          <Cell
            className="md:col-span-2"
            delay={0.16}
            title="Checks your server"
            body="When you add a server, Valo tests each part of it and tells you what's missing. One click copies a report for whoever runs it."
          >
            <CheckPreview />
          </Cell>
          <Cell
            className="md:col-span-6"
            title="Small and fast"
            body="Written in Rust with FFmpeg, drawn on the GPU in step with your screen, so video doesn't tear. It updates itself on Windows, as an AppImage, as a Flatpak and on Android."
          >
            <FastPreview />
          </Cell>
        </div>
      </div>
    </section>
  )
}
