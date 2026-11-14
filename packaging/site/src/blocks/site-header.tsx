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
    <header className="pointer-events-none fixed inset-x-0 top-0 z-50 flex justify-center px-3 pt-3 sm:pt-4">
      <motion.div
        initial={{ y: -24, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ duration: 0.6, ease: [0.2, 0.7, 0.2, 1] }}
        className={cn(
          "pointer-events-auto relative flex h-14 w-full items-center gap-2 overflow-hidden rounded-full border pr-2 pl-4 transition-[max-width,background-color,border-color,box-shadow] duration-500",
          scrolled
            ? "max-w-3xl border-white/10 bg-black/55 shadow-[0_10px_40px_-10px_rgba(0,0,0,.8)] backdrop-blur-xl"
            : "max-w-6xl border-transparent bg-transparent",
        )}
      >
        <a href="#top" className="group mr-auto flex items-center gap-2.5" aria-label="Valo home">
          <img src="./valo.svg" alt="" className="size-7 transition-transform duration-700 group-hover:rotate-[360deg]" />
          <img src="./valo-text-white.svg" alt="Valo" className="h-4" />
        </a>

        <nav className="hidden items-center md:flex" aria-label="Sections">
          {links.map((l) => (
            <a
              key={l.id}
              href={`#${l.id}`}
              className={cn(
                "relative rounded-full px-4 py-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground",
                active === l.id && "text-foreground",
              )}
            >
              {active === l.id && (
                <motion.span
                  layoutId="nav-pill"
                  className="absolute inset-0 -z-10 rounded-full bg-white/[0.08]"
                  transition={{ type: "spring", stiffness: 400, damping: 32 }}
                />
              )}
              {l.label}
            </a>
          ))}
        </nav>

        <a href={GITHUB} className="hidden rounded-full p-2 text-muted-foreground transition-colors hover:text-foreground md:block" aria-label="Valo on GitHub">
          <GitHubIcon className="size-5" />
        </a>

        <Button asChild className="hidden h-10 rounded-full px-4 sm:inline-flex">
          <a href="#download">
            <DownloadIcon data-icon="inline-start" />
            Download
          </a>
        </Button>

        <Sheet>
          <SheetTrigger asChild>
            <Button variant="ghost" size="icon-lg" className="rounded-full md:hidden" aria-label="Menu">
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

        {/* how far down the page */}
        <motion.div
          className="absolute inset-x-6 bottom-0 h-px origin-left bg-gradient-to-r from-transparent via-primary to-transparent"
          style={{ scaleX: progress, opacity: scrolled ? 1 : 0 }}
        />
      </motion.div>
    </header>
  )
}
