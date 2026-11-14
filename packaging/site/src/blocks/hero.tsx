import { ArrowRightIcon, DownloadIcon } from "lucide-react"
import { motion } from "motion/react"
import { GitHubIcon, PlatformIcon } from "@/components/brand-icons"
import { AnimatedShinyText } from "@/components/ui/animated-shiny-text"
import { BlurFade } from "@/components/ui/blur-fade"
import { Button } from "@/components/ui/button"
import { DotPattern } from "@/components/ui/dot-pattern"
import { WordRotate } from "@/components/ui/word-rotate"
import { allFiles, platforms, useDetected, type ReleaseInfo } from "@/downloads"
import { cn } from "@/lib/utils"
import { GITHUB, RELEASES } from "@/release"
import { AppWindow } from "./app-window"

export function Hero({ rel }: { rel: ReleaseInfo }) {
  const detected = useDetected()
  const file = detected?.kind === "file" ? allFiles.find((f) => f.id === detected.fileId) : undefined

  return (
    <section id="top" className="relative overflow-hidden pt-14 pb-20 sm:pt-20 lg:pb-28">
      {/* background: slow aurora over a dot grid */}
      <div className="pointer-events-none absolute inset-0 -z-10">
        <div className="animate-aurora absolute -top-40 left-[8%] size-[560px] rounded-full bg-[#4a7cf7]/25 blur-[110px]" />
        <div className="animate-aurora absolute -top-24 right-[4%] size-[460px] rounded-full bg-violet-500/15 blur-[110px] [animation-delay:-6s]" />
        <div className="animate-aurora absolute top-[40%] left-[45%] size-[380px] rounded-full bg-orange-400/[0.07] blur-[110px] [animation-delay:-12s]" />
        <DotPattern
          width={22}
          height={22}
          className="text-white/[0.07] [mask-image:radial-gradient(ellipse_70%_60%_at_50%_30%,#000_30%,transparent_75%)]"
        />
      </div>

      <div className="mx-auto grid max-w-6xl items-center gap-14 px-4 sm:px-6 lg:grid-cols-[1.05fr_1fr]">
        <div>
          <BlurFade delay={0.05}>
            <a
              href={rel.tag ? `${RELEASES}/tag/${rel.tag}` : `${RELEASES}/latest`}
              className="group mb-7 inline-flex items-center rounded-full border border-white/10 bg-white/[0.04] text-sm transition-colors hover:bg-white/[0.08]"
            >
              <AnimatedShinyText className="inline-flex items-center gap-2 px-3.5 py-1 transition ease-out">
                <span className="relative flex size-2">
                  <span className="absolute inline-flex size-full animate-ping rounded-full bg-emerald-400 opacity-60" />
                  <span className="relative inline-flex size-2 rounded-full bg-emerald-400" />
                </span>
                {rel.version ? `Valo ${rel.version} is out` : "Free and open source"}
                <ArrowRightIcon className="size-3.5 transition-transform duration-300 group-hover:translate-x-0.5" />
              </AnimatedShinyText>
            </a>
          </BlurFade>

          <BlurFade delay={0.12}>
            <h1 className="text-[clamp(2.5rem,5.6vw,4rem)] leading-[1.04] font-semibold tracking-[-0.03em] text-balance">
              Your movies and shows,
              <span className="flex flex-wrap items-baseline gap-x-3">
                <WordRotate
                  words={["played beautifully.", "on every screen.", "from your server.", "right where you left off."]}
                  duration={2800}
                  className="bg-gradient-to-r from-[#6a93fa] via-violet-300 to-[#6a93fa] bg-clip-text text-transparent"
                />
              </span>
            </h1>
          </BlurFade>

          <BlurFade delay={0.2}>
            <p className="mt-2 mb-9 max-w-xl text-lg text-pretty text-muted-foreground sm:text-xl">
              Valo is a small, fast video player for your own media library. Point it at the computer that keeps your
              movies and series, and browse them like a streaming service: posters, Continue watching, seasons and
              episodes, all from your own server.
            </p>
          </BlurFade>

          <BlurFade delay={0.28}>
            <div className="flex flex-wrap items-center gap-3">
              <Button asChild size="lg" className="group relative h-12 overflow-hidden rounded-full px-6 text-base shadow-[0_0_40px_-8px_rgba(74,124,247,.8)]">
                <a href={file ? rel.href(file) : "#download"}>
                  <span className="absolute inset-0 -translate-x-full bg-gradient-to-r from-transparent via-white/25 to-transparent transition-transform duration-700 group-hover:translate-x-full" />
                  <DownloadIcon data-icon="inline-start" className="size-5" />
                  {detected && detected.kind !== "unsupported" ? `Download for ${detected.label}` : "Download Valo"}
                </a>
              </Button>
              <Button asChild size="lg" variant="outline" className="h-12 rounded-full px-6 text-base">
                <a href={GITHUB}>
                  <GitHubIcon data-icon="inline-start" className="size-5" />
                  Source on GitHub
                </a>
              </Button>
            </div>
            <p className="mt-3 h-5 text-sm text-muted-foreground">
              {detected?.kind === "unsupported" && `There's no build for ${detected.label} yet. `}
              {detected?.kind === "flatpak" && "Installs with Flatpak. "}
              <a href="#download" className="underline-offset-4 hover:text-foreground hover:underline">
                All downloads
              </a>
            </p>
          </BlurFade>

          <BlurFade delay={0.36}>
            <ul className="mt-8 flex flex-wrap gap-2">
              {platforms.map((p) => (
                <li
                  key={p.id}
                  className={cn(
                    "inline-flex items-center gap-2 rounded-full border border-white/10 bg-white/[0.03] px-3 py-1 text-sm text-muted-foreground",
                    detected?.kind === "file" && detected.platform === p.id && "border-primary/50 text-foreground",
                    detected?.kind === "flatpak" && p.id === "linux" && "border-primary/50 text-foreground",
                  )}
                >
                  <PlatformIcon id={p.id} className="size-4" />
                  {p.id === "android" ? "Android TV & phones" : p.name}
                </li>
              ))}
            </ul>
          </BlurFade>
        </div>

        <motion.div className="relative" initial={false}>
          <AppWindow />
        </motion.div>
      </div>
    </section>
  )
}
