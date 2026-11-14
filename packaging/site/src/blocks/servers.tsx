import { FolderTreeIcon, PlugIcon, ServerIcon } from "lucide-react"
import { SectionHeading } from "@/components/section-heading"
import { BlurFade } from "@/components/ui/blur-fade"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { AnimatedSpan, Terminal, TypingAnimation } from "@/components/ui/terminal"
import { GITHUB } from "@/release"

const routes = [
  ["GET /api/info", "name, and whether sign-in is needed"],
  ["POST /api/login", "a token, when it is"],
  ["POST /api/logout", "optional"],
  ["GET /api/library", "movies and series, with stream URLs"],
]

export function Servers() {
  return (
    <section id="servers" className="relative py-24 sm:py-28">
      <div className="pointer-events-none absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-white/15 to-transparent" />
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <SectionHeading index="03" kicker="Servers" title={<>Bring <em className="text-primary">your own</em> server.</>}>
          Valo plays from a computer on your network that keeps your movies and shows. There are three ways to set one up.
        </SectionHeading>

        <div className="grid grid-cols-1 gap-4 lg:grid-cols-[1.2fr_1fr]">
          <BlurFade inView className="lg:row-span-2">
            <Card className="h-full rounded-3xl border-white/[0.07] bg-white/[0.02] ring-0">
              <CardHeader>
                <div className="mb-2 grid size-10 place-items-center rounded-lg bg-primary/10 text-primary">
                  <ServerIcon className="size-5" />
                </div>
                <CardTitle className="text-lg">The bundled server</CardTitle>
                <CardDescription className="text-[15px]">
                  One Python file with everything Valo uses, from sign-in to progress sync. It needs nothing but Python. Point it at a folder
                  with <code className="text-foreground/80">Movies/</code> and <code className="text-foreground/80">Series/</code> inside.
                </CardDescription>
              </CardHeader>
              <CardContent>
                <Terminal className="max-w-none overflow-x-auto rounded-2xl border-white/10 bg-black/50">
                  <TypingAnimation>$ python3 valo_server.py ~/Videos --add-user me</TypingAnimation>
                  <AnimatedSpan className="text-muted-foreground">Password for me:</AnimatedSpan>
                  <AnimatedSpan className="text-muted-foreground">Again:</AnimatedSpan>
                  <AnimatedSpan className="text-emerald-400">User me saved (user). Sign-in is now required.</AnimatedSpan>
                  <TypingAnimation>$ python3 valo_server.py ~/Videos</TypingAnimation>
                  <AnimatedSpan className="text-sky-300">[info] Serving /home/me/Videos on http://0.0.0.0:8080  (sign-in on)</AnimatedSpan>
                </Terminal>
              </CardContent>
            </Card>
          </BlurFade>

          <BlurFade inView delay={0.08}>
            <Card className="h-full rounded-3xl border-white/[0.07] bg-white/[0.02] ring-0">
              <CardHeader>
                <div className="mb-2 grid size-10 place-items-center rounded-lg bg-primary/10 text-primary">
                  <PlugIcon className="size-5" />
                </div>
                <CardTitle className="text-lg">Your own app</CardTitle>
                <CardDescription className="text-[15px]">Already run a media site? Four routes are enough for Valo to show it:</CardDescription>
              </CardHeader>
              <CardContent>
                <dl className="divide-y divide-border text-sm">
                  {routes.map(([r, d]) => (
                    <div key={r} className="flex flex-wrap justify-between gap-x-4 gap-y-0.5 py-2">
                      <dt>
                        <code className="text-foreground/90">{r}</code>
                      </dt>
                      <dd className="text-muted-foreground">{d}</dd>
                    </div>
                  ))}
                </dl>
              </CardContent>
            </Card>
          </BlurFade>

          <BlurFade inView delay={0.16}>
            <Card className="h-full rounded-3xl border-white/[0.07] bg-white/[0.02] ring-0">
              <CardHeader>
                <div className="mb-2 grid size-10 place-items-center rounded-lg bg-primary/10 text-primary">
                  <FolderTreeIcon className="size-5" />
                </div>
                <CardTitle className="text-lg">Any folder listing</CardTitle>
                <CardDescription className="text-[15px]">
                  A plain directory listing from nginx, Apache or Caddy works too. Valo works out movies and series from the folder and file
                  names, and picks up <code className="text-foreground/80">poster.jpg</code> and <code className="text-foreground/80">fanart.jpg</code>{" "}
                  next to them.
                </CardDescription>
              </CardHeader>
            </Card>
          </BlurFade>
        </div>

        <p className="mt-6 text-sm text-muted-foreground">
          The{" "}
          <a className="text-primary underline-offset-4 hover:underline" href={`${GITHUB}/blob/main/docs/api/README.md`}>
            API guide
          </a>{" "}
          covers setup, the server check, every route, and examples for Laravel, Express and Flask.
        </p>
      </div>
    </section>
  )
}
