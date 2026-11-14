import type { ReactNode } from "react"
import { BlurFade } from "@/components/ui/blur-fade"
import { cn } from "@/lib/utils"

export function SectionHeading({
  kicker,
  title,
  children,
  className,
}: {
  kicker: string
  title: ReactNode
  children?: ReactNode
  className?: string
}) {
  return (
    <div className={cn("mb-12 max-w-2xl", className)}>
      <BlurFade inView>
        <p className="mb-3 text-xs font-semibold tracking-[0.18em] text-primary uppercase">{kicker}</p>
      </BlurFade>
      <BlurFade inView delay={0.08}>
        <h2 className="mb-4 text-3xl leading-tight font-semibold tracking-tight text-balance sm:text-4xl">{title}</h2>
      </BlurFade>
      {children && (
        <BlurFade inView delay={0.16}>
          <div className="text-lg text-pretty text-muted-foreground">{children}</div>
        </BlurFade>
      )}
    </div>
  )
}
