import { cn } from "@/lib/utils"

// Made-up posters: a gradient, a soft light and title bars. No real artwork.
export const posterColors = [
  ["#3d5ea8", "#1d2a4f"],
  ["#a8573d", "#4a2216"],
  ["#3da88a", "#16473c"],
  ["#8a3da8", "#351647"],
  ["#a8963d", "#47401a"],
  ["#3d8aa8", "#163947"],
  ["#a83d5e", "#47162a"],
  ["#5e3da8", "#211647"],
  ["#a8783d", "#47301a"],
  ["#3da85a", "#16471f"],
]

export function Poster({ i, className, title = true }: { i: number; className?: string; title?: boolean }) {
  const [a, b] = posterColors[i % posterColors.length]
  const w = 40 + ((i * 37) % 45)
  return (
    <div
      className={cn("relative aspect-[2/3] overflow-hidden rounded-lg", className)}
      style={{ background: `linear-gradient(160deg, ${a}, ${b})` }}
    >
      <div
        className="absolute -top-1/4 left-1/4 size-full rounded-full opacity-40 blur-2xl"
        style={{ background: `radial-gradient(circle, ${a}, transparent 60%)` }}
      />
      {title && (
        <div className="absolute inset-x-[12%] bottom-[10%] flex flex-col gap-[6%]">
          <div className="h-[0.35rem] rounded-full bg-white/80" style={{ width: `${w}%` }} />
          <div className="h-[0.2rem] w-1/3 rounded-full bg-white/35" />
        </div>
      )}
    </div>
  )
}
