import { ArrowDownToLineIcon, BugIcon } from "lucide-react"
import { BlurFade } from "@/components/ui/blur-fade"
import { Button } from "@/components/ui/button"
import { Separator } from "@/components/ui/separator"
import { GITHUB, RELEASES } from "@/release"

export function Closing() {
  return (
    <section className="relative isolate overflow-hidden py-32 text-center">
      <div className="pointer-events-none absolute top-1/2 left-1/2 -z-10 size-[520px] -translate-x-1/2 -translate-y-1/2 rounded-full bg-primary/15 blur-[120px]" />
      <div className="mx-auto max-w-2xl px-4">
        <BlurFade inView>
          <div className="relative mx-auto mb-8 size-24">
            <div className="animate-spin-slow absolute -inset-3 rounded-full bg-[conic-gradient(from_0deg,transparent,#4a7cf7,#a77bf6,transparent_60%)] opacity-70 blur-md" />
            <img src="./valo.svg" alt="" className="relative size-24 drop-shadow-2xl" />
          </div>
        </BlurFade>
        <BlurFade inView delay={0.08}>
          <h2 className="mb-4 font-display text-6xl leading-none sm:text-8xl">
            Press <em className="bg-gradient-to-r from-[#8fb0ff] via-[#c7b5ff] to-[#ffc4a3] bg-clip-text text-transparent">play.</em>
          </h2>
        </BlurFade>
        <BlurFade inView delay={0.16}>
          <p className="mb-10 text-lg text-muted-foreground">Your library, ready to watch. Free and open source, under the MIT or Apache 2.0 license.</p>
        </BlurFade>
        <BlurFade inView delay={0.24}>
          <div className="flex flex-wrap justify-center gap-3">
            <Button asChild size="lg" className="h-12 rounded-full px-6 text-base">
              <a href="#download">
                <ArrowDownToLineIcon data-icon="inline-start" className="size-5" />
                Download Valo
              </a>
            </Button>
            <Button asChild size="lg" variant="outline" className="h-12 rounded-full px-6 text-base">
              <a href={`${GITHUB}/issues`}>
                <BugIcon data-icon="inline-start" className="size-5" />
                Report a problem
              </a>
            </Button>
          </div>
        </BlurFade>
      </div>
    </section>
  )
}

export function SiteFooter() {
  const links = [
    ["GitHub", GITHUB],
    ["Releases", RELEASES],
    ["API guide", `${GITHUB}/blob/main/docs/api/README.md`],
    ["Issues", `${GITHUB}/issues`],
  ]
  return (
    <footer className="border-t border-border">
      <div className="mx-auto flex max-w-6xl flex-wrap items-center gap-x-6 gap-y-3 px-4 py-8 text-sm text-muted-foreground sm:px-6">
        <span className="flex flex-1 items-center gap-2.5">
          <img src="./valo.svg" alt="" className="size-5" />
          Valo
          <Separator orientation="vertical" className="h-4!" />
          MIT OR Apache-2.0
        </span>
        {links.map(([l, h]) => (
          <a key={l} href={h} className="transition-colors hover:text-foreground">
            {l}
          </a>
        ))}
      </div>
    </footer>
  )
}
