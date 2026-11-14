import { MotionConfig } from "motion/react"
import { Closing, SiteFooter } from "@/blocks/closing"
import { Download } from "@/blocks/download"
import { Features, Specs } from "@/blocks/features"
import { Hero } from "@/blocks/hero"
import { Servers } from "@/blocks/servers"
import { SiteHeader } from "@/blocks/site-header"
import { Toaster } from "@/components/ui/sonner"
import { TooltipProvider } from "@/components/ui/tooltip"
import { useRelease } from "@/downloads"

export default function App() {
  const rel = useRelease()
  return (
    // reducedMotion="user": motion's animations respect the system setting too
    <MotionConfig reducedMotion="user">
      <TooltipProvider delayDuration={200}>
        <SiteHeader />
        <main>
          <Hero rel={rel} />
          <Specs />
          <Features />
          <Download rel={rel} />
          <Servers />
          <Closing />
        </main>
        <SiteFooter />
        <Toaster position="bottom-center" />
      </TooltipProvider>
    </MotionConfig>
  )
}
