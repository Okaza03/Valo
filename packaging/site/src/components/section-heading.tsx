import type { ReactNode } from "react"
import { BlurFade } from "@/components/ui/blur-fade"
import { cn } from "@/lib/utils"

export function SectionHeading({
  index,
  kicker,
  title,
  children,
  className,
}: {
  index?: string
  kicker: string
  title: ReactNode
  children?: ReactNode
  className?: string
}) {
  return (
    <div className={cn("mb-14 max-w-3xl", className)}>
      <BlurFade inView>
        <p className="mb-5 flex items-center gap-3 font-mono text-xs tracking-[0.16em] text-muted-foreground uppercase [.text-center_&]:justify-center">
          {index && <span className="text-primary">{index}</span>}
          <span className="h-px w-8 bg-white/20" />
          {kicker}
        </p>
      </BlurFade>
      <BlurFade inView delay={0.08}>
        <h2 className="font-display text-[2.6rem] leading-[1.02] text-balance sm:text-6xl [&_em]:italic">{title}</h2>
      </BlurFade>
      {children && (
        <BlurFade inView delay={0.16}>
          <div className="mt-5 text-lg text-pretty text-muted-foreground">{children}</div>
        </BlurFade>
      )}
    </div>
  )
}
