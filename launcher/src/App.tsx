import * as React from "react"
import { listen } from "@tauri-apps/api/event"
import { BadgeCheck, CircleAlert, CircleCheck, FolderOpen, Gauge, Gem, Images, MessageSquareHeart, Rocket, Save, ScrollText } from "lucide-react"
import { toast } from "sonner"

import { useAsk } from "@/components/ask"
import { Backdrop, type Profile, Sidebar } from "@/components/shell"
import { Button } from "@/components/ui/button"
import { Toaster } from "@/components/ui/sonner"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip"
import { type Key, useI18n } from "@/i18n"
import { type Facts, type GameCheck, type JobKind, type Snapshot, JOB_KEYS, api, shortGpu } from "@/lib/api"
import { type ActionId, type Step, computeSteps, todo } from "@/lib/steps"
import { HelpPage } from "@/pages/help"
import { PlayPage } from "@/pages/play"
import { PreparePage } from "@/pages/prepare"
import { SettingsPage } from "@/pages/settings"

export type Tab = "play" | "prepare" | "settings" | "help"

const TITLES: Record<Tab, Key> = { play: "nav.play", prepare: "nav.prepare", settings: "nav.settings", help: "nav.help" }

/** Everything the pages read and can do. */
export type Ctx = {
  snap: Snapshot
  facts: Facts | null
  steps: Step[]
  running: JobKind | null
  lines: string[]
  lastLine: string
  /** The game's once-a-second line: frames per second and ms per frame. */
  fps: { fps: number; ms: number } | null
  /** « Detected profile: Series S · RTX 3050 · 8 GB · 12 threads · 32 GB RAM » (and « checking the card… »). */
  profileLine: string
  /** The same without the pending note: the Settings card's title. */
  profileTitle: string
  captures: string[]
  captureNow: () => Promise<void>
  preparing: { done: number; total: number; current: string } | null
  go: (tab: Tab) => void
  act: (id: ActionId) => Promise<boolean>
  play: () => Promise<void>
  prepareAll: () => Promise<void>
  stop: () => void
  setMode: (mode: string) => Promise<void>
  /** An option of Settings; turning frame generation on fetches its files when they are missing. */
  setOption: (name: "framegen" | "petite_vram", on: boolean) => Promise<void>
  /** NVIDIA's Streamline files for frame generation (series-s/framegen.ps1). */
  getFramegenFiles: () => Promise<boolean>
  pickDlss: () => Promise<void>
  open: (target: string) => void
  backup: () => Promise<void>
  refresh: () => Promise<Snapshot>
  refreshFacts: () => Promise<Facts | null>
}

const MAX_LINES = 1500

export default function App() {
  const i18n = useI18n()
  const { t, msg, err, data } = i18n
  // The event listeners below are set once: they read the language of now through this.
  const i18nRef = React.useRef(i18n)
  i18nRef.current = i18n

  const [tab, setTab] = React.useState<Tab>("play")
  const [snap, setSnap] = React.useState<Snapshot | null>(null)
  const [facts, setFacts] = React.useState<Facts | null>(null)
  const [running, setRunning] = React.useState<JobKind | null>(null)
  const [lines, setLines] = React.useState<string[]>([])
  const [fps, setFps] = React.useState<Ctx["fps"]>(null)
  const [preparing, setPreparing] = React.useState<Ctx["preparing"]>(null)
  const [loadError, setLoadError] = React.useState<string | null>(null)
  const [captures, setCaptures] = React.useState<string[]>([])
  const [collapsed, setCollapsed] = React.useState(false)
  const { ask, dialog } = useAsk()

  const refresh = React.useCallback(async () => {
    const s = await api.snapshot()
    setSnap(s)
    return s
  }, [])
  const refreshFacts = React.useCallback(async () => {
    try {
      const f = await api.facts()
      setFacts(f)
      // The hardware profile depends on the graphics card: read again now that it is known.
      await api.snapshot().then(setSnap).catch(() => {})
      return f
    } catch (e) {
      const now = i18nRef.current
      toast.error(now.t("toast.checksFailed", { error: now.err(e) }))
      return null
    }
  }, [])

  const refreshCaptures = React.useCallback(() => {
    api.captures().then(setCaptures).catch(() => setCaptures([]))
  }, [])

  React.useEffect(() => {
    refresh().catch((e) => setLoadError(i18nRef.current.err(e)))
    refreshFacts()
    refreshCaptures()
    const subs = [
      listen<string>("capture", () => refreshCaptures()),
      listen<{ kind: JobKind; line: string }>("job-line", (e) => {
        const { kind, line } = e.payload
        if (kind === "play") {
          // « 9.8 fps, 102.0 ms/frame (...) » once a second: the numbers are enough.
          const m = line.match(/([\d.]+) fps, ([\d.]+) ms\/frame/)
          if (m) setFps({ fps: Number(m[1]), ms: Number(m[2]) })
        }
        setLines((prev) => {
          const next = prev.length >= MAX_LINES ? prev.slice(prev.length - MAX_LINES + 1) : prev.slice()
          next.push(line)
          return next
        })
      }),
      listen<{ kind: JobKind }>("job-start", (e) => {
        setRunning(e.payload.kind)
        setFps(null)
        const title = i18nRef.current.t(JOB_KEYS[e.payload.kind])
        setLines((prev) => [...prev.slice(-MAX_LINES + 1), `── ${title}`])
      }),
      listen<{ kind: JobKind; code: number }>("job-end", (e) => {
        setRunning(null)
        const ended = i18nRef.current.t("job.ended", { code: e.payload.code })
        setLines((prev) => [...prev.slice(-MAX_LINES + 1), `── ${ended}`])
        refresh()
      }),
    ]
    return () => subs.forEach((p) => p.then((un) => un()))
  }, [refresh, refreshFacts, refreshCaptures])

  // The language saved in lanceur.json, once, when the first snapshot arrives (English when absent).
  const langSynced = React.useRef(false)
  React.useEffect(() => {
    if (!snap || langSynced.current) return
    langSynced.current = true
    const saved = snap.settings.langue === "fr" ? "fr" : "en"
    if (saved !== i18n.lang) i18n.setLang(saved, false)
  }, [snap, i18n])

  const steps = React.useMemo(() => (snap ? computeSteps(snap, facts, i18n) : []), [snap, facts, i18n])

  // --preview <dir> (scripts/package-release.ps1 -Screenshots): every page in turn, saved as launcher-<name>.png,
  // once the checks are done; then the window closes.
  const previewStarted = React.useRef(false)
  React.useEffect(() => {
    if (!snap || !facts || previewStarted.current) return
    previewStarted.current = true
    api.previewMode().then(async (on) => {
      if (!on) return
      const pages: [Tab, string][] = [["play", "jouer"], ["prepare", "preparation"], ["settings", "reglages"], ["help", "aide"]]
      const wait = (ms: number) => new Promise((r) => window.setTimeout(r, ms))
      await document.fonts.ready
      for (const [page, name] of pages) {
        setTab(page)
        await wait(2500)
        await api.previewShot(name).catch((e) => console.error(e))
      }
      await api.previewDone()
    })
  }, [snap, facts])

  // ---------------------------------------------------------------- actions

  const runJob = async (kind: JobKind): Promise<boolean> => {
    try {
      const code = await api.runJob(kind)
      return code === 0
    } catch (e) {
      toast.error(err(e))
      return false
    }
  }

  const getFramegenFiles = async (): Promise<boolean> => {
    const ok = await runJob("framegen")
    await refresh()
    if (ok) toast.success(t("toast.framegenFiles"))
    else toast.error(t("toast.framegenFilesFailed"))
    return ok
  }

  const pickGame = () => api.pickGame(t("dialog.pickGame.title"), t("dialog.pickGame.filter"))

  const findGame = async (): Promise<boolean> => {
    const id = toast.loading(t("toast.searchingGame"))
    let all: GameCheck[] = []
    try {
      all = await api.findGames()
    } finally {
      toast.dismiss(id)
    }
    // Complete copies only (the exe alone or a few archives do not run); the Rust side lists them first.
    const found = all.filter((g) => g.complete).map((g) => g.path)
    let chosen: string | null = null
    const firstProblem = all[0]?.problem
    if (found.length === 0 && firstProblem) toast.error(msg(firstProblem), { duration: 15000 })
    if (found.length === 1) chosen = found[0]
    else if (found.length > 1) {
      chosen = await ask({
        title: t("ask.copies.title", { count: found.length }),
        body: <p>{t("ask.copies.body")}</p>,
        choices: [
          ...found.slice(0, 4).map((f) => ({ label: f.replace(/\\game_scarlett_master\.exe$/i, ""), value: f, variant: "secondary" as const })),
          { label: t("common.cancel"), value: "", variant: "ghost" as const },
        ],
      })
    }
    try {
      if (chosen) {
        await api.setGame(chosen)
        toast.success(t("toast.gameFound", { path: chosen }))
      } else {
        if (all.length === 0) toast.info(t("toast.gameNotFound"))
        chosen = await pickGame()
      }
    } catch (e) {
      // An incomplete copy: what is missing, long enough to be read.
      toast.error(err(e), { duration: 15000 })
      chosen = null
    }
    await refresh()
    return !!chosen
  }

  const nvCache = async (): Promise<boolean> => {
    if (facts && !facts.nvidia) return true
    const opened = await api.nvidiaPanel()
    const r = await ask({
      title: t("nvcache.title"),
      body: (
        <>
          <p>{t("nvcache.why")}</p>
          <p>
            {opened ? t("nvcache.opened") : t("nvcache.open")} {t("nvcache.howTo")}
          </p>
          <p>{t("nvcache.driver")}</p>
        </>
      ),
      choices: [
        { label: t("nvcache.yes"), value: "yes" },
        { label: t("nvcache.no"), value: "no", variant: "ghost" },
      ],
    })
    if (r === null) return false
    await api.confirmNvCache(r === "yes")
    await refresh()
    return r === "yes"
  }

  const power = async (action: "high" | "revert"): Promise<boolean> => {
    try {
      toast.success(msg(await api.power(action)))
      await refreshFacts()
      await refresh()
      return true
    } catch (e) {
      toast.error(err(e))
      return false
    }
  }

  const act = async (id: ActionId): Promise<boolean> => {
    switch (id) {
      case "rust":
      case "vs": {
        const ok = await runJob(id)
        await refreshFacts()
        return ok
      }
      case "build":
        return runJob(id)
      case "shaders": {
        // The shared pipeline list first: the translation keeps the shaders it names while it walks them, so
        // step 4 can build its pipelines before the first game.
        const s = await api.snapshot()
        if (s.sharedList && !(await runJob("import"))) toast.warning(t("toast.importFailed"))
        return runJob("shaders")
      }
      case "warm": {
        // Shaders translated before the list was imported: import it now with --shaders (~7 min), then build.
        const s = await api.snapshot()
        if (s.sharedList && s.settings.shadersReady && s.settings.pipelinesImport !== s.sharedList.mark && !(await runJob("import")))
          return false
        return runJob("warm")
      }
      case "find_game":
        return findGame()
      case "pick_game":
        try {
          const p = await pickGame()
          await refresh()
          return !!p
        } catch (e) {
          toast.error(err(e), { duration: 15000 })
          return false
        }
      case "nv_cache":
        return nvCache()
      case "power_high":
        return power("high")
      case "power_revert":
        return power("revert")
    }
  }

  const prepareAll = async () => {
    if (!snap) return
    const auto = steps.filter((s) => s.auto)
    if (auto.length === 0) {
      toast.success(t("toast.allReady"))
      return
    }
    const r = await ask({
      title: t("prepareAll.title"),
      body: (
        <>
          <p>{t("prepareAll.intro")}</p>
          <ul className="flex list-none flex-col gap-1.5 pl-0">
            {auto.map((s) => (
              <li key={s.n} className="flex gap-2">
                <span className="w-4 font-bold text-white">{s.n}</span>
                <span>{s.action?.label ?? s.title}</span>
              </li>
            ))}
          </ul>
          <p>{t("prepareAll.note")}</p>
        </>
      ),
      choices: [
        { label: t("common.start"), value: "go" },
        { label: t("common.cancel"), value: "", variant: "ghost" },
      ],
    })
    if (r !== "go") return
    const order: ActionId[] = ["rust", "vs", "build", "find_game", "shaders", "warm", "power_high"]
    const total = auto.length
    let done = 0
    for (const id of order) {
      // Re-read the state before each step: an earlier one may have changed it.
      const s = await refresh()
      const f = (await refreshFacts()) ?? facts
      const step = computeSteps(s, f, i18n).find((x) => x.auto === id)
      if (!step) continue
      setPreparing({ done, total, current: step.action?.label ?? step.title })
      const ok = await act(id)
      done++
      if (!ok) {
        setPreparing(null)
        toast.error(t("toast.prepareStopped", { n: step.n, title: step.title }))
        return
      }
    }
    setPreparing(null)
    await refresh()
    toast.success(t("toast.prepareDone"))
    if (facts?.nvidia && !snap.settings.nvCacheOk) await nvCache()
  }

  const play = async () => {
    if (!snap) return
    if (snap.exeBuilt == null) {
      toast.warning(t("toast.notBuilt"))
      setTab("prepare")
      return
    }
    if (!snap.gameOk && !(await findGame())) return
    if (!snap.settings.shadersReady) {
      const r = await ask({
        title: t("noShaders.title"),
        body: <p>{t("noShaders.body")}</p>,
        choices: [
          { label: t("noShaders.prepare"), value: "prep" },
          { label: t("noShaders.play"), value: "play", variant: "ghost" },
        ],
      })
      if (r !== "play") {
        if (r === "prep") setTab("prepare")
        return
      }
    }
    const ok = await runJob("play")
    await refresh()
    if (!ok) toast.info(t("toast.playError"))
  }

  // The profile's label in the window's language: « RTX 3050 · 8 GB · 12 threads · 32 GB RAM ».
  const hw = snap?.hardware
  const hwLabel = hw
    ? [
        hw.gpu || t("profile.unknownCard"),
        hw.vramGb > 0 ? t("unit.gb", { n: hw.vramGb }) : "",
        hw.threads > 0 ? t("profile.threads", { count: hw.threads }) : "",
        hw.ramGb > 0 ? t("profile.ramGb", { n: hw.ramGb }) : "",
      ]
        .filter(Boolean)
        .join(" · ")
    : ""
  const profileTitle = t("profile.detected", { label: hwLabel })
  const profileLine = hw
    ? profileTitle + (hw.pending ? t("profile.checkingCard") : "")
    : [
        t("profile.seriesS"),
        facts ? (facts.gpuName ? shortGpu(facts.gpuName) : t("profile.unknownCard")) : "…",
        snap ? t("unit.gb", { n: i18n.gb(snap.ramBytes) }) : "…",
      ].join(" · ")

  const ctx: Ctx | null = snap && {
    snap,
    facts,
    steps,
    running,
    lines,
    lastLine: lines[lines.length - 1] ?? "",
    fps,
    profileLine,
    profileTitle,
    captures,
    captureNow: async () => {
      try {
        const f = await api.captureNow()
        toast.success(t("toast.captureSaved", { file: f.split(/[\\/]/).pop() ?? f }))
        refreshCaptures()
      } catch (e) {
        toast.error(err(e))
      }
    },
    preparing,
    go: setTab,
    act,
    play,
    prepareAll,
    stop: () => api.stopJob(),
    setMode: async (mode) => {
      try {
        await api.setMode(mode)
      } catch (e) {
        toast.error(err(e))
      }
      await refresh()
    },
    setOption: async (name, on) => {
      try {
        await api.setOption(name, on)
      } catch (e) {
        toast.error(err(e))
      }
      const s = await refresh()
      // Frame generation turned on without its files: fetch them now (NVIDIA's release, ~70 MB).
      if (name === "framegen" && on && s.framegen && s.framegen.kind !== "" && !s.framegen.filesOk) await getFramegenFiles()
    },
    getFramegenFiles,
    pickDlss: async () => {
      try {
        if (await api.pickDlss(t("dialog.pickDlss.title"))) toast.success(t("toast.dlssSaved"))
      } catch (e) {
        toast.error(err(e))
      }
      await refresh()
    },
    open: (target) => api.open(target).catch((e) => toast.error(err(e))),
    backup: async () => {
      try {
        toast.success(t("toast.savesCopied", { path: await api.backupSaves() }))
      } catch (e) {
        toast.error(err(e))
      }
    },
    refresh,
    refreshFacts,
  }

  // Overall readiness for the dot next to the navigation.
  const pending = todo(steps).length
  const essentials = !!snap && snap.exeBuilt != null && snap.gameOk
  const readiness = !snap
    ? { color: "text-muted-foreground", ok: false, text: t("status.checking") }
    : !essentials
      ? { color: "text-destructive", ok: false, text: t("status.notReady") }
      : pending > 0
        ? { color: "text-warning", ok: false, text: t("status.stepsLeft", { count: pending }) }
        : { color: "text-success", ok: true, text: t("status.ready") }

  // Profiles in the sidebar: the Series S settings, alone or with a DLSS mode.
  const profiles: Profile[] = snap
    ? [
        { id: "off", icon: BadgeCheck, name: t("profile.recommended") },
        // DLSS: NVIDIA RTX cards only (hardware profile); the rows wait for the Streamline folder.
        ...(snap.hardware && !snap.hardware.dlss ? [] : snap.dlssModes).map((m) => ({
          id: m.id,
          icon: m.id === "ultra" ? Rocket : m.id === "performance" ? Gauge : Gem,
          name: data(`dlss.${m.id}.nom`, m.nom),
          disabledWhy: snap.dlssOk ? undefined : t("profile.dlssNeedsFolder"),
        })),
      ]
    : []
  const selected =
    snap && snap.dlssOk && snap.hardware?.dlss !== false && snap.dlssModes.some((m) => m.id === snap.settings.dlssMode) ? snap.settings.dlssMode : "off"

  const link = "uppercase transition-colors duration-200 hover:text-white"

  return (
    <TooltipProvider delayDuration={300}>
      <div className="flex h-full">
        {snap && ctx && (
          <Sidebar
            snap={snap}
            profiles={profiles}
            selected={selected}
            onSelect={(id) => running !== "play" && ctx.setMode(id)}
            collapsed={collapsed}
            onCollapse={() => setCollapsed((c) => !c)}
            shortcuts={[
              { label: t("shortcut.lastLog"), icon: ScrollText, run: () => ctx.open("last_log"), disabled: !snap.lastLog },
              { label: t("shortcut.logs"), icon: FolderOpen, run: () => ctx.open("logs") },
              { label: t("shortcut.captures"), icon: Images, run: () => ctx.open("captures") },
              { label: t("shortcut.backup"), icon: Save, run: ctx.backup },
            ]}
          />
        )}

        <Tabs value={tab} onValueChange={(v) => setTab(v as Tab)} className="relative min-w-0 flex-1 flex-col">
          <Backdrop files={captures} mode={tab === "play" ? "hero" : "dim"} />

          <header className="relative flex shrink-0 items-end justify-between gap-8 px-10 pt-6 pb-2">
            <div className="flex min-w-0 flex-col gap-1">
              <h1 className="text-[34px] leading-tight font-bold">{t(TITLES[tab])}</h1>
            </div>
            <div className="flex items-center gap-8 pb-1.5">
              <TabsList>
                <TabsTrigger value="play">{t("nav.play")}</TabsTrigger>
                <TabsTrigger value="prepare">{t("nav.prepare")}</TabsTrigger>
                <TabsTrigger value="settings">{t("nav.settings")}</TabsTrigger>
                <TabsTrigger value="help">{t("nav.help")}</TabsTrigger>
              </TabsList>
              <Tooltip>
                <TooltipTrigger asChild>
                  <button
                    type="button"
                    onClick={() => setTab("prepare")}
                    className="flex size-9 items-center justify-center rounded-full border border-border bg-[#2a2a2a] outline-none focus-visible:ring-2 focus-visible:ring-white/60"
                    aria-label={readiness.text}
                  >
                    {readiness.ok ? (
                      <CircleCheck className={`size-[18px] ${readiness.color}`} strokeWidth={1.75} />
                    ) : (
                      <CircleAlert className={`size-[18px] ${readiness.color}`} strokeWidth={1.75} />
                    )}
                  </button>
                </TooltipTrigger>
                <TooltipContent side="bottom" align="end">
                  {readiness.text}
                </TooltipContent>
              </Tooltip>
            </div>
          </header>

          <main className="scroll-thin relative min-h-0 flex-1 overflow-y-auto">
            <div className="flex min-h-full flex-col px-10 pt-6 pb-8">
              {loadError && <p className="text-destructive">{loadError}</p>}
              {ctx && (
                <>
                  <TabsContent value="play" className="flex flex-1 flex-col">
                    <PlayPage ctx={ctx} />
                  </TabsContent>
                  <TabsContent value="prepare">
                    <PreparePage ctx={ctx} />
                  </TabsContent>
                  <TabsContent value="settings">
                    <SettingsPage ctx={ctx} />
                  </TabsContent>
                  <TabsContent value="help">
                    <HelpPage ctx={ctx} />
                  </TabsContent>
                </>
              )}
            </div>
          </main>

          {running && running !== "play" && (
            <div className="relative flex shrink-0 items-center gap-4 border-t border-border bg-card px-10 py-2.5">
              <span className="size-2 shrink-0 animate-pulse rounded-full bg-warning" />
              <span className="shrink-0 text-[12px] font-bold">{t(JOB_KEYS[running])}</span>
              <span className="min-w-0 flex-1 truncate font-mono text-[11.5px] text-muted-foreground">
                {lines[lines.length - 1]}
              </span>
              <Button variant="secondary" size="sm" onClick={() => api.stopJob()}>
                {t("common.stop")}
              </Button>
            </div>
          )}

          <footer className="relative flex shrink-0 items-center gap-6 border-t border-white/8 px-10 py-3.5 text-[11px] font-semibold tracking-[0.08em] text-[#8a8a8a]">
            <span>© VIctory S{snap ? ` ${snap.version}` : ""}</span>
            <button type="button" className={link} onClick={() => ctx?.open("docs")}>
              {t("footer.docs")}
            </button>
            <button type="button" className={link} onClick={() => ctx?.open("license")}>
              {t("footer.license")}
            </button>
            <button type="button" className={link} onClick={() => ctx?.open("issues")}>
              {t("footer.report")}
            </button>
            <button type="button" className={`${link} ml-auto flex items-center gap-1.5 text-white/85`} onClick={() => ctx?.open("feedback")}>
              <MessageSquareHeart className="size-3.5" strokeWidth={2} />
              {t("footer.feedback")}
            </button>
          </footer>
        </Tabs>
      </div>
      {dialog}
      <Toaster position="bottom-right" />
    </TooltipProvider>
  )
}
