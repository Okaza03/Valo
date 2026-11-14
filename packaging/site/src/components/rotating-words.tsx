import { AnimatePresence, motion, useReducedMotion } from "motion/react"
import { useEffect, useState } from "react"
import { cn } from "@/lib/utils"

/**
 * Cycles through `words` without moving anything around it: every word sits
 * in the same grid cell (the hidden ones keep their space), so the box is
 * always as wide and tall as the longest word, and words never wrap.
 */
export function RotatingWords({
  words,
  interval = 2600,
  className,
  wordClassName,
}: {
  words: string[]
  interval?: number
  className?: string
  /** on the visible word itself: a gradient (bg-clip-text) has to sit there, not on a parent */
  wordClassName?: string
}) {
  const reduce = useReducedMotion()
  const [i, setI] = useState(0)
  useEffect(() => {
    if (reduce) return
    const t = setInterval(() => setI((n) => (n + 1) % words.length), interval)
    return () => clearInterval(t)
  }, [reduce, interval, words.length])

  return (
    <span className={cn("inline-grid whitespace-nowrap", className)}>
      {/* sizers: invisible, one per word, all in the same cell */}
      {words.map((w) => (
        <span key={w} aria-hidden="true" className="invisible [grid-area:1/1]">
          {w}
        </span>
      ))}
      <span className="sr-only">{words[0]}</span>
      <AnimatePresence mode="popLayout" initial={false}>
        <motion.span
          key={words[i]}
          aria-hidden="true"
          className={cn("[grid-area:1/1]", wordClassName)}
          initial={{ opacity: 0, y: "0.45em", filter: "blur(8px)" }}
          animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
          exit={{ opacity: 0, y: "-0.45em", filter: "blur(8px)" }}
          transition={{ duration: 0.55, ease: [0.2, 0.7, 0.2, 1] }}
        >
          {words[i]}
        </motion.span>
      </AnimatePresence>
    </span>
  )
}
