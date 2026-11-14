import type { SVGProps } from "react"
import type { PlatformId } from "@/downloads"

type P = SVGProps<SVGSVGElement>

export function GitHubIcon(props: P) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" {...props}>
      <path d="M12 .5a11.5 11.5 0 0 0-3.64 22.41c.58.1.79-.25.79-.56v-2c-3.2.7-3.88-1.37-3.88-1.37-.53-1.33-1.28-1.69-1.28-1.69-1.05-.71.08-.7.08-.7 1.16.08 1.77 1.19 1.77 1.19 1.03 1.77 2.7 1.26 3.36.96.1-.75.4-1.26.73-1.55-2.55-.29-5.24-1.28-5.24-5.68 0-1.26.45-2.28 1.19-3.09-.12-.29-.52-1.46.11-3.04 0 0 .97-.31 3.17 1.18a11 11 0 0 1 5.77 0c2.2-1.49 3.17-1.18 3.17-1.18.63 1.58.23 2.75.11 3.04.74.81 1.19 1.83 1.19 3.09 0 4.41-2.69 5.39-5.25 5.67.41.36.78 1.06.78 2.14v3.17c0 .31.21.67.8.56A11.5 11.5 0 0 0 12 .5Z" />
    </svg>
  )
}

export function WindowsIcon(props: P) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" {...props}>
      <path d="M3 5.5 10.5 4.4v7.1H3zm0 13 7.5 1.1v-7H3zm8.5 1.2L21 21v-8.5h-9.5zm0-15.4v7.2H21V3z" />
    </svg>
  )
}

export function LinuxIcon(props: P) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" {...props}>
      <path d="M12 2.5c-2.3 0-3.6 1.9-3.6 4.4 0 1.5.3 2.6-.6 4.1-1 1.7-2.6 3.6-2.6 6 0 1.2.5 2 1.4 2.5" />
      <path d="M12 2.5c2.3 0 3.6 1.9 3.6 4.4 0 1.5-.3 2.6.6 4.1 1 1.7 2.6 3.6 2.6 6 0 1.2-.5 2-1.4 2.5" />
      <path d="M8.4 19.6c1 .9 2.2 1.4 3.6 1.4s2.6-.5 3.6-1.4" />
      <circle cx="10.4" cy="7.4" r=".6" fill="currentColor" />
      <circle cx="13.6" cy="7.4" r=".6" fill="currentColor" />
      <path d="M10.6 9.6 12 10.6l1.4-1" />
    </svg>
  )
}

export function AndroidIcon(props: P) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" {...props}>
      <path d="M4 16.5a8 8 0 0 1 16 0z" />
      <path d="M7 6.5 8.6 9M17 6.5 15.4 9" />
      <circle cx="9" cy="13" r=".7" fill="currentColor" />
      <circle cx="15" cy="13" r=".7" fill="currentColor" />
    </svg>
  )
}

export function PlatformIcon({ id, ...props }: P & { id: PlatformId }) {
  if (id === "windows") return <WindowsIcon {...props} />
  if (id === "linux") return <LinuxIcon {...props} />
  return <AndroidIcon {...props} />
}
