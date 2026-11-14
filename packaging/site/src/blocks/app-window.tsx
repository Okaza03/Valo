import { AnimatePresence, motion, useMotionValue, useReducedMotion, useSpring, useTransform } from "motion/react"
import { useEffect, useRef, useState } from "react"
import { BorderBeam } from "@/components/ui/border-beam"

// An illustrated Valo window: shapes, not a screenshot. The featured banner
// cycles, Continue watching fills in, and a focus ring walks along the
// posters like a TV remote would.

const banners = [
  { bg: "linear-gradient(135deg,#2b3f7c 0%,#6c3f8f 45%,#c46a4a 100%)", t: 150, s: 110 },
  { bg: "linear-gradient(135deg,#16473c 0%,#2f7d6b 50%,#a8963d 100%)", t: 120, s: 140 },
  { bg: "linear-gradient(135deg,#47162a 0%,#8a3da8 55%,#3d5ea8 100%)", t: 170, s: 90 },
]

const posters = [
  "linear-gradient(160deg,#3d5ea8,#1d2a4f)",
  "linear-gradient(160deg,#8a3da8,#351647)",
  "linear-gradient(160deg,#a8963d,#47401a)",
  "linear-gradient(160deg,#a83d5e,#47162a)",
  "linear-gradient(160deg,#3da88a,#16473c)",
  "linear-gradient(160deg,#a8573d,#4a2216)",
  "linear-gradient(160deg,#3d8aa8,#163947)",
]

const continuing = [
  { bg: "linear-gradient(160deg,#a8573d,#4a2216)", p: 62 },
  { bg: "linear-gradient(160deg,#3d8aa8,#163947)", p: 28 },
  { bg: "linear-gradient(160deg,#3da88a,#16473c)", p: 81 },
]

export function AppWindow() {
  const reduce = useReducedMotion()
  const [banner, setBanner] = useState(0)
  const [focus, setFocus] = useState(1)

  useEffect(() => {
    if (reduce) return
    const a = setInterval(() => setBanner((b) => (b + 1) % banners.length), 4200)
    const b = setInterval(() => setFocus((f) => (f + 1) % 6), 1400)
    return () => {
      clearInterval(a)
      clearInterval(b)
    }
  }, [reduce])

  // tilt toward the pointer
  const ref = useRef<HTMLDivElement>(null)
  const mx = useMotionValue(0)
  const my = useMotionValue(0)
  const rx = useSpring(useTransform(my, [-0.5, 0.5], [7, -7]), { stiffness: 120, damping: 18 })
  const ry = useSpring(useTransform(mx, [-0.5, 0.5], [-10, 10]), { stiffness: 120, damping: 18 })

  function onMove(e: React.PointerEvent) {
    if (reduce || e.pointerType !== "mouse") return
    const r = ref.current!.getBoundingClientRect()
    mx.set((e.clientX - r.left) / r.width - 0.5)
    my.set((e.clientY - r.top) / r.height - 0.5)
  }
  function onLeave() {
    mx.set(0)
    my.set(0)
  }

  const b = banners[banner]

  return (
    <div ref={ref} onPointerMove={onMove} onPointerLeave={onLeave} className="[perspective:1400px]" aria-hidden="true">
      <motion.div
        style={{ rotateX: rx, rotateY: ry }}
        initial={{ opacity: 0, y: 40, rotateY: -14 }}
        animate={{ opacity: 1, y: 0, rotateY: -6 }}
        transition={{ duration: 1, ease: [0.2, 0.7, 0.2, 1], delay: 0.25 }}
        className="relative overflow-hidden rounded-xl border border-white/10 bg-[#0b0c0f] shadow-[0_40px_120px_-20px_rgba(0,0,0,.75),0_0_80px_-30px_rgba(74,124,247,.5)] [transform-style:preserve-3d]"
      >
        <BorderBeam size={120} duration={9} colorFrom="#6a93fa" colorTo="#b38bfa" borderWidth={1.5} />

        {/* top bar */}
        <div className="flex h-9 items-center gap-3.5 border-b border-white/5 bg-[#121317] px-3.5">
          <img src="./valo-text-white.svg" alt="" className="h-2.5 opacity-90" />
          <div className="h-1.5 w-9 rounded-full bg-white/90" />
          <div className="h-1.5 w-9 rounded-full bg-white/25" />
          <div className="h-1.5 w-9 rounded-full bg-white/25" />
          <div className="ml-auto h-4 w-24 rounded border border-white/10" />
        </div>

        {/* featured banner */}
        <div className="relative h-40 overflow-hidden sm:h-44">
          <AnimatePresence initial={false}>
            <motion.div
              key={banner}
              className="absolute inset-0"
              style={{ background: b.bg }}
              initial={{ opacity: 0, scale: 1.08 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 1.1, ease: "easeOut" }}
            />
          </AnimatePresence>
          <div className="absolute inset-0 bg-gradient-to-r from-[#0b0c0f] from-15% via-[#0b0c0f]/40 to-transparent" />
          <div className="absolute inset-x-0 bottom-0 h-10 bg-gradient-to-t from-[#0b0c0f] to-transparent" />
          <AnimatePresence mode="wait">
            <motion.div
              key={banner}
              className="absolute bottom-4 left-4 flex flex-col gap-2"
              initial={{ opacity: 0, x: -14 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: 8 }}
              transition={{ duration: 0.45 }}
            >
              <div className="h-4 rounded-sm bg-white" style={{ width: b.t }} />
              <div className="h-1.5 rounded-full bg-white/45" style={{ width: b.s }} />
              <div className="mt-1.5 flex gap-2">
                <div className="flex h-5 w-[70px] items-center gap-1.5 rounded-full bg-white pl-2.5">
                  <div className="size-0 border-y-[4px] border-l-[6px] border-y-transparent border-l-black" />
                  <div className="h-1 w-7 rounded-full bg-black/70" />
                </div>
                <div className="h-5 w-14 rounded-full bg-white/15" />
              </div>
            </motion.div>
          </AnimatePresence>
          <div className="absolute right-4 bottom-4 flex gap-1.5">
            {banners.map((_, i) => (
              <div key={i} className="h-1 w-4 overflow-hidden rounded-full bg-white/20">
                {i === banner && (
                  <motion.div
                    key={banner}
                    className="h-full bg-white"
                    initial={{ width: reduce ? "100%" : 0 }}
                    animate={{ width: "100%" }}
                    transition={{ duration: 4.2, ease: "linear" }}
                  />
                )}
              </div>
            ))}
          </div>
        </div>

        {/* Continue watching */}
        <div className="mx-4 mt-3 mb-2 h-1.5 w-24 rounded-full bg-white/25" />
        <div className="flex gap-2 px-4">
          {continuing.map((c, i) => (
            <div key={i} className="relative h-16 w-28 shrink-0 overflow-hidden rounded-md" style={{ background: c.bg }}>
              <div className="absolute inset-x-1.5 bottom-1.5 h-[3px] rounded-full bg-white/25">
                <div className="animate-fill h-full rounded-full bg-[#4a7cf7]" style={{ width: `${c.p}%`, animationDelay: `${0.9 + i * 0.2}s` }} />
              </div>
            </div>
          ))}
        </div>

        {/* posters, with the remote's focus ring */}
        <div className="mx-4 mt-3.5 mb-2 h-1.5 w-20 rounded-full bg-white/25" />
        <div className="flex gap-2 px-4 pb-5">
          {posters.slice(0, 7).map((bg, i) => (
            <motion.div
              key={i}
              className="relative h-[86px] w-[58px] shrink-0 rounded-md"
              style={{ background: bg }}
              animate={{ scale: focus === i ? 1.08 : 1, y: focus === i ? -3 : 0 }}
              transition={{ type: "spring", stiffness: 300, damping: 22 }}
            >
              {focus === i && (
                <motion.div
                  layoutId="poster-focus"
                  className="absolute -inset-[3px] rounded-[9px] ring-2 ring-white shadow-[0_8px_24px_rgba(0,0,0,.5)]"
                  transition={{ type: "spring", stiffness: 300, damping: 28 }}
                />
              )}
            </motion.div>
          ))}
        </div>
      </motion.div>
    </div>
  )
}
