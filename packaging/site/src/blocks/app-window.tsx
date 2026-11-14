import { AnimatePresence, motion, useReducedMotion } from "motion/react"
import { useEffect, useState } from "react"
import { Poster } from "@/components/poster"
import { BorderBeam } from "@/components/ui/border-beam"

// An illustrated Valo window: shapes, not a screenshot. The featured banner
// cycles, Continue watching fills in, and a focus ring walks along the
// posters like a TV remote would.

const banners = [
  { bg: "linear-gradient(120deg,#1b2a5c 0%,#5a3384 45%,#c46a4a 100%)", t: "44%", s: "30%" },
  { bg: "linear-gradient(120deg,#0f3a31 0%,#2f7d6b 50%,#c9a447 100%)", t: "36%", s: "38%" },
  { bg: "linear-gradient(120deg,#3a0f22 0%,#8a3da8 55%,#3d5ea8 100%)", t: "50%", s: "26%" },
]

const continuing = [
  { bg: "linear-gradient(160deg,#a8573d,#4a2216)", p: 62 },
  { bg: "linear-gradient(160deg,#3d8aa8,#163947)", p: 28 },
  { bg: "linear-gradient(160deg,#3da88a,#16473c)", p: 81 },
  { bg: "linear-gradient(160deg,#8a3da8,#351647)", p: 45 },
]

export function AppWindow() {
  const reduce = useReducedMotion()
  const [banner, setBanner] = useState(0)
  const [focus, setFocus] = useState(2)

  useEffect(() => {
    if (reduce) return
    const a = setInterval(() => setBanner((b) => (b + 1) % banners.length), 4800)
    const b = setInterval(() => setFocus((f) => (f + 1) % 8), 1300)
    return () => {
      clearInterval(a)
      clearInterval(b)
    }
  }, [reduce])

  const b = banners[banner]

  return (
    <div
      aria-hidden="true"
      className="relative overflow-hidden rounded-2xl border border-white/10 bg-[#0b0c0f] shadow-[0_50px_140px_-30px_rgba(0,0,0,.9),0_0_120px_-40px_rgba(74,124,247,.55)]"
    >
      <BorderBeam size={180} duration={10} colorFrom="#6a93fa" colorTo="#b38bfa" borderWidth={1.5} />

      {/* top bar: wordmark, Home / Movies / TV, search */}
      <div className="flex h-11 items-center gap-5 border-b border-white/5 bg-[#101115] px-5">
        <img src="./valo-text-white.svg" alt="" className="h-3 opacity-90" />
        <div className="flex gap-4 text-[11px] font-medium">
          <span className="text-white">Home</span>
          <span className="text-white/40">Movies</span>
          <span className="text-white/40">TV</span>
          <span className="hidden text-white/40 sm:inline">My list</span>
        </div>
        <div className="ml-auto h-6 w-32 rounded-md border border-white/10 sm:w-44" />
      </div>

      {/* featured banner */}
      <div className="relative h-48 overflow-hidden sm:h-64 md:h-72">
        <AnimatePresence initial={false}>
          <motion.div
            key={banner}
            className="absolute inset-0"
            style={{ background: b.bg }}
            initial={{ opacity: 0, scale: 1.1 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 1.4, ease: "easeOut" }}
          />
        </AnimatePresence>
        <div className="absolute inset-0 bg-gradient-to-r from-[#0b0c0f] from-10% via-[#0b0c0f]/50 via-45% to-transparent" />
        <div className="absolute inset-x-0 bottom-0 h-16 bg-gradient-to-t from-[#0b0c0f] to-transparent" />
        <AnimatePresence mode="wait">
          <motion.div
            key={banner}
            className="absolute bottom-7 left-6 flex w-[60%] flex-col gap-2.5 sm:left-8"
            initial={{ opacity: 0, x: -18 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: 10 }}
            transition={{ duration: 0.5 }}
          >
            <div className="h-1.5 w-16 rounded-full bg-[#6a93fa]" />
            <div className="h-5 rounded bg-white sm:h-7" style={{ width: b.t }} />
            <div className="h-2 rounded-full bg-white/40" style={{ width: b.s }} />
            <div className="h-2 w-[22%] rounded-full bg-white/25" />
            <div className="mt-2 flex gap-2">
              <div className="flex h-7 items-center gap-2 rounded-full bg-white px-3.5 text-[11px] font-semibold text-black">
                <span className="size-0 border-y-[5px] border-l-[8px] border-y-transparent border-l-black" />
                Play
              </div>
              <div className="flex h-7 items-center rounded-full bg-white/15 px-3.5 text-[11px] font-medium text-white">More info</div>
            </div>
          </motion.div>
        </AnimatePresence>
        <div className="absolute right-6 bottom-7 flex gap-1.5">
          {banners.map((_, i) => (
            <div key={i} className="h-1 w-6 overflow-hidden rounded-full bg-white/20">
              {i === banner && (
                <motion.div
                  key={banner}
                  className="h-full bg-white"
                  initial={{ width: reduce ? "100%" : 0 }}
                  animate={{ width: "100%" }}
                  transition={{ duration: 4.8, ease: "linear" }}
                />
              )}
            </div>
          ))}
        </div>
      </div>

      {/* Continue watching */}
      <p className="mx-6 mt-2 mb-2.5 text-[11px] font-semibold text-white/70 sm:mx-8">Continue watching</p>
      <div className="flex gap-3 px-6 sm:px-8">
        {continuing.map((c, i) => (
          <div key={i} className="relative aspect-video w-36 shrink-0 overflow-hidden rounded-lg sm:w-44" style={{ background: c.bg }}>
            <div className="absolute inset-x-2 bottom-2 h-[3px] rounded-full bg-white/25">
              <div className="animate-fill h-full rounded-full bg-[#4a7cf7]" style={{ width: `${c.p}%`, animationDelay: `${0.8 + i * 0.15}s` }} />
            </div>
          </div>
        ))}
      </div>

      {/* posters, with the remote's focus ring */}
      <p className="mx-6 mt-5 mb-2.5 text-[11px] font-semibold text-white/70 sm:mx-8">Recently added</p>
      <div className="flex gap-3 px-6 pb-7 sm:px-8">
        {Array.from({ length: 9 }, (_, i) => (
          <motion.div
            key={i}
            className="relative w-[72px] shrink-0 sm:w-[92px]"
            animate={{ scale: focus === i ? 1.08 : 1, y: focus === i ? -4 : 0 }}
            transition={{ type: "spring", stiffness: 300, damping: 22 }}
          >
            <Poster i={i + 3} title={false} className="rounded-md" />
            {focus === i && (
              <motion.div
                layoutId="poster-focus"
                className="absolute -inset-[3px] rounded-[9px] shadow-[0_10px_30px_rgba(0,0,0,.6)] ring-2 ring-white"
                transition={{ type: "spring", stiffness: 300, damping: 28 }}
              />
            )}
          </motion.div>
        ))}
      </div>
    </div>
  )
}
