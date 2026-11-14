import {
  CaptionsIcon,
  HistoryIcon,
  LibraryBigIcon,
  PaletteIcon,
  ShieldCheckIcon,
  ZapIcon,
  type LucideIcon,
} from "lucide-react"
import { BlurFade } from "@/components/ui/blur-fade"
import { MagicCard } from "@/components/ui/magic-card"
import { Marquee } from "@/components/ui/marquee"
import { NumberTicker } from "@/components/ui/number-ticker"
import { SectionHeading } from "@/components/section-heading"
import { cn } from "@/lib/utils"

const features: { icon: LucideIcon; title: string; body: string; wide?: boolean; tint: string }[] = [
  {
    icon: LibraryBigIcon,
    title: "A real library",
    body: "A home page with a featured banner and rows for Continue watching, My list, Recently added and every genre. Movie and TV grids sort and filter, and search finds titles, genres and directors.",
    wide: true,
    tint: "from-[#4a7cf7]/25",
  },
  {
    icon: HistoryIcon,
    title: "Picks up where you left off",
    body: "Valo remembers where you stopped and plays the next episode for you. Your progress syncs with the server, so another device carries on from the same spot.",
    tint: "from-emerald-500/20",
  },
  {
    icon: CaptionsIcon,
    title: "Subtitles that fit",
    body: "Subtitles from the server, or found by your server's subtitle addons. Your language is remembered, and if a file is a little off you can shift the timing while you watch.",
    tint: "from-amber-500/20",
  },
  {
    icon: PaletteIcon,
    title: "Your look",
    body: "Seven styles change how the home page, cards, details pages and player are laid out, and six color themes include light, black for OLED screens, and following your desktop.",
    tint: "from-violet-500/25",
  },
  {
    icon: ShieldCheckIcon,
    title: "Checks your server",
    body: "When you add a server, Valo tests each part of it, from sign-in to streaming and artwork, and tells you what's missing. One click copies a report for whoever runs it.",
    tint: "from-sky-500/20",
  },
  {
    icon: ZapIcon,
    title: "Small and fast",
    body: "Written in Rust with FFmpeg, drawn on the GPU in step with your screen, so video doesn't tear. It updates itself with one click on Windows, as an AppImage and as a Flatpak.",
    wide: true,
    tint: "from-rose-500/20",
  },
]

const chips = [
  "Continue watching",
  "My list",
  "Recently added",
  "Next episode",
  "Subtitle timing",
  "Subtitle addons",
  "Progress sync",
  "Seasons and episodes",
  "Search by director",
  "OLED black theme",
  "Chromecast",
  "Server check",
  "GPU video, no tearing",
  "Self-updating",
]

const stats = [
  { value: 4, label: "platforms", suffix: "" },
  { value: 7, label: "layout styles", suffix: "" },
  { value: 6, label: "color themes", suffix: "" },
  { value: 10, label: "ready-made downloads", suffix: "" },
]

export function Stats({ downloads }: { downloads: number }) {
  return (
    <div className="border-y border-border bg-black/20">
      <div className="mx-auto grid max-w-6xl grid-cols-2 divide-border px-4 sm:px-6 md:grid-cols-4 md:divide-x">
        {(downloads > 0
          ? [...stats.slice(0, 3), { value: downloads, label: "downloads of this release", suffix: "" }]
          : stats
        ).map((s, i) => (
          <BlurFade key={s.label} inView delay={i * 0.06} className="px-2 py-7 text-center md:px-6">
            <div className="text-3xl font-semibold tracking-tight tabular-nums sm:text-4xl">
              <NumberTicker value={s.value} className="text-foreground" />
              {s.suffix}
            </div>
            <div className="mt-1 text-sm text-muted-foreground">{s.label}</div>
          </BlurFade>
        ))}
      </div>
    </div>
  )
}

export function Features() {
  return (
    <section id="features" className="relative py-24 sm:py-28">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <SectionHeading kicker="Features" title="Everything you'd expect from a streaming app, for the files you own.">
          Valo doesn't host anything. It plays straight from your own server, on your own network.
        </SectionHeading>

        <div className="grid gap-4 md:grid-cols-3">
          {features.map((f, i) => (
            <BlurFade key={f.title} inView delay={0.05 + (i % 3) * 0.08} className={cn(f.wide && "md:col-span-2")}>
              <MagicCard
                className="group h-full rounded-2xl"
                gradientFrom="#4a7cf7"
                gradientTo="#a77bf6"
                gradientColor="rgba(74,124,247,0.12)"
                gradientSize={260}
              >
                <div className="relative h-full overflow-hidden p-6 sm:p-7">
                  <div
                    className={cn(
                      "pointer-events-none absolute -top-16 -right-16 size-48 rounded-full bg-gradient-to-br to-transparent opacity-0 blur-2xl transition-opacity duration-500 group-hover:opacity-100",
                      f.tint,
                    )}
                  />
                  <div className="mb-5 grid size-11 place-items-center rounded-xl border border-primary/25 bg-primary/10 text-primary transition-transform duration-500 group-hover:-rotate-6 group-hover:scale-110">
                    <f.icon className="size-5" />
                  </div>
                  <h3 className="mb-2 text-lg font-semibold">{f.title}</h3>
                  <p className={cn("text-[15px] leading-relaxed text-muted-foreground", f.wide && "max-w-xl")}>{f.body}</p>
                </div>
              </MagicCard>
            </BlurFade>
          ))}
        </div>
      </div>

      <div className="relative mt-16 [mask-image:linear-gradient(90deg,transparent,#000_12%,#000_88%,transparent)]">
        <Marquee pauseOnHover className="[--duration:45s] [--gap:0.75rem]">
          {chips.map((c) => (
            <span key={c} className="rounded-full border border-white/10 bg-white/[0.03] px-4 py-2 text-sm whitespace-nowrap text-muted-foreground">
              {c}
            </span>
          ))}
        </Marquee>
        <Marquee reverse pauseOnHover className="[--duration:55s] [--gap:0.75rem]">
          {[...chips].reverse().map((c) => (
            <span key={c} className="rounded-full border border-white/10 bg-white/[0.03] px-4 py-2 text-sm whitespace-nowrap text-muted-foreground">
              {c}
            </span>
          ))}
        </Marquee>
      </div>
    </section>
  )
}
