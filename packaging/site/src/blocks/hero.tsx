import { ArrowDownToLineIcon, ArrowRightIcon } from "lucide-react"
import { motion, useScroll, useSpring, useTransform } from "motion/react"
import { useRef } from "react"
import { GitHubIcon } from "@/components/brand-icons"
import { Poster } from "@/components/poster"
import { RotatingWords } from "@/components/rotating-words"
import { BlurFade } from "@/components/ui/blur-fade"
import { Button } from "@/components/ui/button"
import { allFiles, useDetected, type ReleaseInfo } from "@/downloads"
import { GITHUB, RELEASES } from "@/release"
import { AppWindow } from "./app-window"

// Seven columns of posters on a tilted plane, drifting in turns up and down.
function PosterWall() {
  return (
    <div className="pointer-events-none absolute inset-0 -z-10 overflow-hidden" aria-hidden="true">
      <div className="absolute inset-x-[-20%] top-[-30%] h-[160%] [perspective:1200px]">
        <div className="grid h-full grid-cols-5 gap-4 opacity-[0.55] [transform:rotateX(38deg)_rotateZ(-10deg)] sm:grid-cols-7">
          {Array.from({ length: 7 }, (_, col) => (
            <div
              key={col}
              className={col % 2 ? "animate-drift-down" : "animate-drift-up"}
              style={{ ["--drift" as string]: `${70 + col * 9}s` }}
            >
              {/* twice over, so the -50% loop is seamless */}
              {[0, 1].map((half) => (
                <div key={half} className="flex flex-col gap-4 pb-4">
                  {Array.from({ length: 6 }, (_, k) => (
                    <Poster key={k} i={col * 3 + k} />
                  ))}
                </div>
              ))}
            </div>
          ))}
        </div>
      </div>
      {/* fade the wall into the page so the text stays readable */}
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_60%_55%_at_50%_38%,var(--background)_25%,transparent_80%)] opacity-90" />
      <div className="absolute inset-0 bg-gradient-to-b from-background/70 via-background/40 to-background" />
      <div className="absolute top-[18%] left-1/2 size-[640px] -translate-x-1/2 rounded-full bg-[#4a7cf7]/15 blur-[140px]" />
    </div>
  )
}

export function Hero({ rel }: { rel: ReleaseInfo }) {
  const detected = useDetected()
  const file = detected?.kind === "file" ? allFiles.find((f) => f.id === detected.fileId) : undefined

  // the window lies back and stands up as it scrolls into view
  const shot = useRef<HTMLDivElement>(null)
  const { scrollYProgress } = useScroll({ target: shot, offset: ["start end", "start 25%"] })
  const p = useSpring(scrollYProgress, { stiffness: 120, damping: 30, mass: 0.4 })
  const rotateX = useTransform(p, [0, 1], [28, 0])
  const scale = useTransform(p, [0, 1], [0.88, 1])
  const y = useTransform(p, [0, 1], [60, 0])

  return (
    <section id="top" className="grain relative isolate overflow-hidden pt-32 sm:pt-40">
      <PosterWall />

      <div className="mx-auto flex max-w-4xl flex-col items-center px-4 text-center sm:px-6">
        <BlurFade delay={0.05}>
          <a
            href={rel.tag ? `${RELEASES}/tag/${rel.tag}` : `${RELEASES}/latest`}
            className="group mb-8 inline-flex items-center gap-2.5 rounded-full border border-white/10 bg-black/40 py-1 pr-3 pl-1 text-sm text-muted-foreground backdrop-blur-md transition-colors hover:border-white/20 hover:text-foreground"
          >
            <span className="rounded-full bg-primary/20 px-2 py-0.5 font-mono text-[11px] font-medium text-primary">
              {rel.version ? `v${rel.version}` : "new"}
            </span>
            {rel.version ? "Read what's new" : "Free and open source"}
            <ArrowRightIcon className="size-3.5 transition-transform group-hover:translate-x-0.5" />
          </a>
        </BlurFade>

        <BlurFade delay={0.12}>
          <h1 className="font-display text-[2.6rem] leading-[0.98] tracking-[-0.01em] sm:text-7xl md:text-[5.5rem]">
            <span className="block">Your movies and shows,</span>
            <RotatingWords
              className="italic"
              wordClassName="bg-gradient-to-r from-[#8fb0ff] via-[#c7b5ff] to-[#ffc4a3] bg-clip-text pr-[0.06em] pb-[0.08em] text-transparent"
              words={["played beautifully.", "on every screen.", "from your server.", "where you left off."]}
            />
          </h1>
        </BlurFade>

        <BlurFade delay={0.2}>
          <p className="mt-7 max-w-2xl text-lg text-pretty text-muted-foreground sm:text-xl">
            Valo is a small, fast video player for your own media library. Browse the movies and series on your own server like a
            streaming service: posters, Continue watching, seasons and episodes.
          </p>
        </BlurFade>

        <BlurFade delay={0.28}>
          <div className="mt-10 flex flex-wrap items-center justify-center gap-3">
            <Button asChild size="lg" className="group relative h-12 overflow-hidden rounded-full px-7 text-base shadow-[0_0_50px_-10px_rgba(74,124,247,.9)]">
              <a href={file ? rel.href(file) : "#download"}>
                <span className="absolute inset-0 -translate-x-full bg-gradient-to-r from-transparent via-white/25 to-transparent transition-transform duration-700 group-hover:translate-x-full" />
                <ArrowDownToLineIcon data-icon="inline-start" className="size-5" />
                {detected && detected.kind !== "unsupported" ? `Download for ${detected.label}` : "Download Valo"}
              </a>
            </Button>
            <Button asChild size="lg" variant="outline" className="h-12 rounded-full border-white/15 bg-black/30 px-7 text-base backdrop-blur-md">
              <a href={GITHUB}>
                <GitHubIcon data-icon="inline-start" className="size-5" />
                Star on GitHub
              </a>
            </Button>
          </div>
          <p className="mt-4 font-mono text-xs tracking-wide text-muted-foreground/80 uppercase">
            {detected?.kind === "unsupported" ? `No ${detected.label} build yet · ` : ""}
            Free · Open source · Your own server
          </p>
        </BlurFade>
      </div>

      <div ref={shot} className="mx-auto mt-16 max-w-6xl px-3 [perspective:1600px] sm:mt-20 sm:px-6">
        <motion.div style={{ rotateX, scale, y, transformOrigin: "50% 0%" }} initial={{ opacity: 0 }} animate={{ opacity: 1 }} transition={{ delay: 0.4, duration: 0.8 }}>
          <AppWindow />
        </motion.div>
      </div>
      <div className="pointer-events-none relative z-10 -mt-32 h-32 bg-gradient-to-t from-background to-transparent" />
    </section>
  )
}
