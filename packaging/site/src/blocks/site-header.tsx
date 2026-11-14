import { DownloadIcon, MenuIcon } from "lucide-react"
import { motion, useScroll, useSpring } from "motion/react"
import { useEffect, useState } from "react"
import { GitHubIcon } from "@/components/brand-icons"
import { Button } from "@/components/ui/button"
import { Sheet, SheetClose, SheetContent, SheetHeader, SheetTitle, SheetTrigger } from "@/components/ui/sheet"
import { cn } from "@/lib/utils"
import { GITHUB } from "@/release"

const links = [
  { id: "features", label: "Features" },
  { id: "download", label: "Download" },
  { id: "servers", label: "Servers" },
]

/** The section in the middle of the screen, for the nav's highlight. */
function useActiveSection() {
  const [active, setActive] = useState<string | null>(null)
  useEffect(() => {
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) if (e.isIntersecting) setActive(e.target.id)
      },
      { rootMargin: "-45% 0px -50% 0px" },
    )
    for (const l of links) {
      const el = document.getElementById(l.id)
      if (el) io.observe(el)
    }
    const top = document.getElementById("top")
    if (top) io.observe(top)
    return () => io.disconnect()
  }, [])
  return active
}

export function SiteHeader() {
  const { scrollYProgress, scrollY } = useScroll()
  const progress = useSpring(scrollYProgress, { stiffness: 140, damping: 30, mass: 0.3 })
  const [scrolled, setScrolled] = useState(false)
  useEffect(() => scrollY.on("change", (y) => setScrolled(y > 12)), [scrollY])
  const active = useActiveSection()

  return (
    <header
      className={cn(
        "sticky top-0 z-50 border-b border-transparent transition-[background-color,border-color,backdrop-filter] duration-300",
        scrolled && "border-border bg-background/75 backdrop-blur-xl",
      )}
    >
      <div className="mx-auto flex h-16 max-w-6xl items-center gap-6 px-4 sm:px-6">
        <a href="#top" className="group flex items-center gap-3" aria-label="Valo home">
          <img src="./valo.svg" alt="" className="size-8 transition-transform duration-500 group-hover:rotate-[360deg]" />
          <img src="./valo-text-white.svg" alt="Valo" className="h-[18px]" />
        </a>

        <nav className="ml-auto hidden items-center gap-1 md:flex" aria-label="Sections">
          {links.map((l) => (
            <a
              key={l.id}
              href={`#${l.id}`}
              className={cn(
                "relative rounded-full px-3.5 py-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground",
                active === l.id && "text-foreground",
              )}
            >
              {active === l.id && (
                <motion.span
                  layoutId="nav-pill"
                  className="absolute inset-0 -z-10 rounded-full bg-muted"
                  transition={{ type: "spring", stiffness: 400, damping: 32 }}
                />
              )}
              {l.label}
            </a>
          ))}
          <a
            href={GITHUB}
            className="ml-1 rounded-full p-2 text-muted-foreground transition-colors hover:text-foreground"
            aria-label="Valo on GitHub"
          >
            <GitHubIcon className="size-5" />
          </a>
        </nav>

        <Button asChild size="lg" className="ml-auto hidden rounded-full px-4 sm:inline-flex md:ml-2">
          <a href="#download">
            <DownloadIcon data-icon="inline-start" />
            Download
          </a>
        </Button>

        <Sheet>
          <SheetTrigger asChild>
            <Button variant="ghost" size="icon-lg" className="ml-auto sm:ml-0 md:hidden" aria-label="Menu">
              <MenuIcon />
            </Button>
          </SheetTrigger>
          <SheetContent side="right" className="w-72">
            <SheetHeader>
              <SheetTitle className="flex items-center gap-2">
                <img src="./valo.svg" alt="" className="size-6" /> Valo
              </SheetTitle>
            </SheetHeader>
            <nav className="flex flex-col gap-1 px-4" aria-label="Sections">
              {[...links, { id: "github", label: "GitHub" }].map((l) => (
                <SheetClose asChild key={l.id}>
                  <a
                    href={l.id === "github" ? GITHUB : `#${l.id}`}
                    className="rounded-lg px-3 py-2.5 text-base text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                  >
                    {l.label}
                  </a>
                </SheetClose>
              ))}
            </nav>
          </SheetContent>
        </Sheet>
      </div>
      <motion.div
        className="absolute inset-x-0 bottom-0 h-px origin-left bg-gradient-to-r from-primary via-violet-400 to-primary"
        style={{ scaleX: progress }}
      />
    </header>
  )
}
