import {
  ArrowDownToLineIcon,
  CalendarIcon,
  FileCheck2Icon,
  ExternalLinkIcon,
  InfoIcon,
  PackageIcon,
  SparklesIcon,
  TerminalIcon,
  TvIcon,
} from "lucide-react"
import { AnimatePresence, motion } from "motion/react"
import { useState } from "react"
import { CopyButton } from "@/components/copy-button"
import { PlatformIcon } from "@/components/brand-icons"
import { SectionHeading } from "@/components/section-heading"
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion"
import { Badge } from "@/components/ui/badge"
import { BlurFade } from "@/components/ui/blur-fade"
import { Button } from "@/components/ui/button"
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"
import { ShineBorder } from "@/components/ui/shine-border"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { allFiles, platforms, useDetected, type Detected, type FileEntry, type PlatformId, type ReleaseInfo } from "@/downloads"
import { cn } from "@/lib/utils"
import { formatSize, release, RELEASES } from "@/release"

const FLATPAK_INSTALL = `flatpak install --user ${release.base}/valo.flatpakref`
const FLATPAK_BY_HAND = [
  "flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo",
  `flatpak remote-add --user --if-not-exists valo ${release.base}/valo.flatpakrepo`,
  "flatpak install --user io.github.okaza03.Valo",
]

function Command({ text, label }: { text: string; label?: string }) {
  return (
    <InputGroup className="h-10 bg-black/30 font-mono">
      <InputGroupAddon>
        <span className="font-mono text-primary select-none">$</span>
      </InputGroupAddon>
      <InputGroupInput readOnly value={text} aria-label={label ?? "Command"} className="font-mono text-[13px]" onFocus={(e) => e.currentTarget.select()} />
      <InputGroupAddon align="inline-end">
        <CopyButton text={text} label="Copy command" />
      </InputGroupAddon>
    </InputGroup>
  )
}

function meta(rel: ReleaseInfo, f: FileEntry) {
  const a = rel.assets.get(rel.name(f))
  return a ? { size: formatSize(a.size), downloads: a.downloads } : null
}

// ---- the card at the top: the one file this visitor most likely wants

function Recommended({ rel, detected, onOthers }: { rel: ReleaseInfo; detected: Detected | null; onOthers: (p: PlatformId) => void }) {
  if (!detected) return <div className="h-[188px] animate-pulse rounded-2xl border border-border bg-card/40" />

  const file = detected.kind === "file" ? allFiles.find((f) => f.id === detected.fileId) : undefined
  const m = file && meta(rel, file)

  return (
    <motion.div
      initial={{ opacity: 0, y: 16, scale: 0.98 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={{ duration: 0.5, ease: [0.2, 0.7, 0.2, 1] }}
      className="relative overflow-hidden rounded-2xl border border-border bg-gradient-to-br from-primary/[0.12] via-card to-card p-6 sm:p-8"
    >
      <ShineBorder shineColor={["#4a7cf7", "#a77bf6", "#6a93fa"]} borderWidth={1} duration={12} />
      <div className="pointer-events-none absolute -top-24 -right-20 size-72 rounded-full bg-primary/20 blur-3xl" />

      <div className="relative flex flex-col gap-6 md:flex-row md:items-center">
        <div className="grid size-16 shrink-0 place-items-center rounded-2xl border border-primary/30 bg-primary/15 text-primary">
          {detected.kind === "unsupported" ? (
            <InfoIcon className="size-8" />
          ) : (
            <PlatformIcon id={detected.kind === "flatpak" ? "linux" : detected.platform} className="size-8" />
          )}
        </div>

        <div className="min-w-0 flex-1">
          <p className="mb-1 flex items-center gap-1.5 text-xs font-semibold tracking-[0.14em] text-primary uppercase">
            <SparklesIcon className="size-3.5" />
            {detected.kind === "unsupported" ? `On ${detected.label}` : `Recommended for ${detected.label}`}
          </p>

          {detected.kind === "file" && file && (
            <>
              <h3 className="text-2xl font-semibold tracking-tight">{file.title}</h3>
              <p className="mt-1 text-muted-foreground">
                {file.detail}
                {m && <span className="text-foreground/70"> · {m.size}</span>}
              </p>
            </>
          )}

          {detected.kind === "flatpak" && (
            <>
              <h3 className="text-2xl font-semibold tracking-tight">Flatpak, in one command</h3>
              <p className="mt-1 mb-4 text-muted-foreground">
                Adds Valo's repository for your user and installs it. After that, <code className="text-foreground/80">flatpak update</code> keeps it
                up to date.
              </p>
              <Command text={FLATPAK_INSTALL} label="Flatpak install command" />
            </>
          )}

          {detected.kind === "unsupported" && (
            <>
              <h3 className="text-2xl font-semibold tracking-tight">No {detected.label} build yet</h3>
              <p className="mt-1 text-muted-foreground">
                Valo runs on Windows, Linux, Android TV and Android phones. Pick one below, or watch the{" "}
                <a className="text-primary underline-offset-4 hover:underline" href={RELEASES}>
                  releases
                </a>{" "}
                for news.
              </p>
            </>
          )}
        </div>

        {detected.kind === "file" && file && (
          <div className="flex shrink-0 flex-col items-stretch gap-2 sm:flex-row sm:items-center md:flex-col md:items-stretch">
            <Button asChild size="lg" className="group h-12 rounded-full px-7 text-base shadow-[0_0_40px_-10px_rgba(74,124,247,.9)]">
              <a href={rel.href(file)}>
                <ArrowDownToLineIcon data-icon="inline-start" className="size-5 transition-transform group-hover:translate-y-0.5" />
                Download
              </a>
            </Button>
            <Button variant="ghost" className="rounded-full text-muted-foreground" onClick={() => onOthers(detected.platform)}>
              Other {platforms.find((p) => p.id === detected.platform)!.name} options
            </Button>
          </div>
        )}
        {detected.kind === "flatpak" && (
          <Button variant="ghost" className="shrink-0 self-start rounded-full text-muted-foreground md:self-center" onClick={() => onOthers("linux")}>
            AppImage, .deb and more
          </Button>
        )}
      </div>
    </motion.div>
  )
}

// ---- one file

function FileRow({ rel, file, index }: { rel: ReleaseInfo; file: FileEntry; index: number }) {
  const m = meta(rel, file)
  return (
    <motion.a
      href={rel.href(file)}
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: index * 0.05, duration: 0.3 }}
      className="group relative flex items-center gap-4 overflow-hidden rounded-xl border border-border bg-background/60 p-4 transition-colors hover:border-primary/60 hover:bg-accent/40"
    >
      <span className="pointer-events-none absolute inset-y-0 left-0 w-px bg-gradient-to-b from-transparent via-primary to-transparent opacity-0 transition-opacity group-hover:opacity-100" />
      <div className="grid size-10 shrink-0 place-items-center rounded-lg bg-muted text-muted-foreground transition-colors group-hover:bg-primary/15 group-hover:text-primary">
        {file.id === "tv" ? <TvIcon className="size-5" /> : <PackageIcon className="size-5" />}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="font-medium">{file.title}</span>
          {file.arch && (
            <Badge variant="outline" className="font-mono text-[10px]">
              {file.arch}
            </Badge>
          )}
          {file.badges?.map((b) => (
            <Badge key={b} variant="secondary" className="text-[10px]">
              {b}
            </Badge>
          ))}
        </div>
        <p className="mt-0.5 truncate text-sm text-muted-foreground">{file.detail}</p>
      </div>
      <div className="hidden shrink-0 text-right text-xs text-muted-foreground tabular-nums sm:block">
        <AnimatePresence>
          {m && (
            <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }}>
              <div className="text-sm text-foreground/80">{m.size}</div>
              <div>{m.downloads.toLocaleString()} downloads</div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
      <Tooltip>
        <TooltipTrigger asChild>
          <span className="grid size-9 shrink-0 place-items-center rounded-full border border-border text-muted-foreground transition-all group-hover:border-primary group-hover:bg-primary group-hover:text-primary-foreground">
            <ArrowDownToLineIcon className="size-4 transition-transform duration-300 group-hover:translate-y-0.5" />
          </span>
        </TooltipTrigger>
        <TooltipContent className="font-mono">{rel.tag ? rel.name(file) : "Opens the release page"}</TooltipContent>
      </Tooltip>
    </motion.a>
  )
}

// ---- the extras under each platform's files

function WindowsNotes() {
  return (
    <p className="flex gap-2 rounded-xl border border-border bg-muted/30 p-4 text-sm text-muted-foreground">
      <InfoIcon className="mt-0.5 size-4 shrink-0" />
      Windows SmartScreen may warn the first time, because Valo isn't code-signed: choose More info, then Run anyway.
    </p>
  )
}

function LinuxRepo() {
  return (
    <div className="rounded-xl border border-primary/30 bg-primary/[0.06] p-5">
      <div className="mb-1 flex items-center gap-2">
        <TerminalIcon className="size-4 text-primary" />
        <h4 className="font-medium">Flatpak repository</h4>
        <Badge className="text-[10px]">Recommended</Badge>
      </div>
      <p className="mb-4 text-sm text-muted-foreground">
        One command adds Valo's repository for your user and installs it. The runtime comes from Flathub, and{" "}
        <code className="text-foreground/80">flatpak update</code> keeps Valo up to date.
      </p>
      <Command text={FLATPAK_INSTALL} label="Flatpak install command" />
      <Accordion type="single" collapsible className="mt-2">
        <AccordionItem value="hand" className="border-none">
          <AccordionTrigger className="py-2 text-sm text-muted-foreground hover:no-underline">Add the repository by hand</AccordionTrigger>
          <AccordionContent className="flex flex-col gap-2">
            {FLATPAK_BY_HAND.map((c) => (
              <Command key={c} text={c} />
            ))}
          </AccordionContent>
        </AccordionItem>
      </Accordion>
    </div>
  )
}

function AndroidSteps() {
  const steps = [
    <>
      On the TV, install the free <b className="text-foreground">Downloader</b> app from the Play Store.
    </>,
    <>Allow Downloader to install apps, in the TV's settings.</>,
    <>Enter the APK's link in Downloader and install it. From then on, Valo updates itself.</>,
  ]
  return (
    <div className="rounded-xl border border-border bg-muted/30 p-5">
      <h4 className="mb-3 flex items-center gap-2 font-medium">
        <TvIcon className="size-4 text-primary" />
        Installing on a TV
      </h4>
      <ol className="flex flex-col gap-3">
        {steps.map((s, i) => (
          <li key={i} className="flex gap-3 text-sm text-muted-foreground">
            <span className="grid size-6 shrink-0 place-items-center rounded-full bg-primary text-xs font-semibold text-primary-foreground">{i + 1}</span>
            <span className="pt-0.5">{s}</span>
          </li>
        ))}
      </ol>
    </div>
  )
}

// ---- the section

export function Download({ rel }: { rel: ReleaseInfo }) {
  const detected = useDetected()
  // the visitor's platform until they pick a tab
  const [chosen, setTab] = useState<PlatformId | null>(null)
  const tab: PlatformId = chosen ?? (detected?.kind === "file" ? detected.platform : detected?.kind === "flatpak" ? "linux" : "windows")

  function showOthers(p: PlatformId) {
    setTab(p)
    document.getElementById("all-downloads")?.scrollIntoView({ behavior: "smooth", block: "start" })
  }

  const date = rel.published ? new Date(rel.published).toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" }) : null

  return (
    <section id="download" className="relative border-y border-border bg-[#0d0e12] py-24 sm:py-28">
      <div className="pointer-events-none absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-primary/60 to-transparent" />
      <div className="mx-auto max-w-5xl px-4 sm:px-6">
        <SectionHeading kicker="Download" title="Get Valo" className="mx-auto text-center">
          Free for Windows, Linux, Android TV and Android phones.
          {rel.version && (
            <>
              {" "}
              This is version <span className="text-foreground">{rel.version}</span>.
            </>
          )}
        </SectionHeading>

        <BlurFade inView>
          <Recommended rel={rel} detected={detected} onOthers={showOthers} />
        </BlurFade>

        <BlurFade inView delay={0.1}>
          <Tabs id="all-downloads" value={tab} onValueChange={(v) => setTab(v as PlatformId)} className="mt-14 scroll-mt-24">
            <div className="mb-6 flex flex-col items-center gap-2">
              <h3 className="text-sm font-medium text-muted-foreground">Every download</h3>
              <TabsList className="h-11! rounded-full bg-muted/60 p-1">
                {platforms.map((p) => (
                  <TabsTrigger key={p.id} value={p.id} className="relative gap-2 rounded-full px-4 data-active:bg-transparent! data-active:shadow-none!">
                    {tab === p.id && (
                      <motion.span
                        layoutId="dl-tab"
                        className="absolute inset-0 -z-0 rounded-full bg-background shadow-sm ring-1 ring-border"
                        transition={{ type: "spring", stiffness: 420, damping: 34 }}
                      />
                    )}
                    <span className="relative z-10 flex items-center gap-2">
                      <PlatformIcon id={p.id} className="size-4" />
                      {p.name}
                    </span>
                  </TabsTrigger>
                ))}
              </TabsList>
            </div>

            {platforms.map((p) => (
              <TabsContent key={p.id} value={p.id} className="mt-0">
                <motion.div
                  key={p.id}
                  initial={{ opacity: 0, y: 8, filter: "blur(4px)" }}
                  animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
                  transition={{ duration: 0.3 }}
                  className="flex flex-col gap-4"
                >
                  <p className="text-center text-sm text-muted-foreground">{p.blurb}</p>
                  {p.id === "linux" && <LinuxRepo />}
                  <div className="flex flex-col gap-2.5">
                    {p.files.map((f, i) => (
                      <FileRow key={f.id} rel={rel} file={f} index={i} />
                    ))}
                  </div>
                  {p.id === "windows" && <WindowsNotes />}
                  {p.id === "android" && <AndroidSteps />}
                </motion.div>
              </TabsContent>
            ))}
          </Tabs>
        </BlurFade>

        <BlurFade inView delay={0.1}>
          <Accordion type="single" collapsible className="mt-8 rounded-xl border border-border bg-background/40 px-5">
            <AccordionItem value="verify" className="border-none">
              <AccordionTrigger className="hover:no-underline">
                <span className="flex items-center gap-2">
                  <FileCheck2Icon className="size-4 text-primary" />
                  Check a download
                </span>
              </AccordionTrigger>
              <AccordionContent className="flex flex-col gap-3 text-muted-foreground">
                <p>
                  Every release has <code className="text-foreground/80">SHA256SUMS.txt</code>, and the Flatpak bundle its own{" "}
                  <code className="text-foreground/80">SHA256SUMS-flatpak.txt</code>. Put it next to what you downloaded and run:
                </p>
                <Command text="sha256sum --check --ignore-missing SHA256SUMS.txt" />
                <p className="text-sm">
                  On Windows, <code className="text-foreground/80">Get-FileHash .\file.exe</code> in PowerShell prints the hash to compare.
                </p>
              </AccordionContent>
            </AccordionItem>
          </Accordion>
        </BlurFade>

        <div className={cn("mt-6 flex flex-wrap items-center justify-center gap-x-5 gap-y-2 text-sm text-muted-foreground")}>
          {rel.version && <span className="text-foreground/80">Version {rel.version}</span>}
          {date && (
            <span className="flex items-center gap-1.5">
              <CalendarIcon className="size-3.5" />
              {date}
            </span>
          )}
          <a className="flex items-center gap-1 hover:text-foreground" href={rel.tag ? `${RELEASES}/tag/${rel.tag}` : `${RELEASES}/latest`}>
            What's new <ExternalLinkIcon className="size-3" />
          </a>
          {rel.tag && (
            <a className="hover:text-foreground" href={`${RELEASES}/download/${rel.tag}/SHA256SUMS.txt`}>
              SHA256SUMS.txt
            </a>
          )}
          <a className="flex items-center gap-1 hover:text-foreground" href={RELEASES}>
            All releases <ExternalLinkIcon className="size-3" />
          </a>
        </div>
      </div>
    </section>
  )
}
