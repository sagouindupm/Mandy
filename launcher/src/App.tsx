import * as React from "react"
import { listen } from "@tauri-apps/api/event"
import { toast } from "sonner"

import { useAsk } from "@/components/ask"
import { Backdrop, type Profile, Sidebar } from "@/components/shell"
import { Button } from "@/components/ui/button"
import { Toaster } from "@/components/ui/sonner"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip"
import { type Facts, type JobKind, type Snapshot, JOB_TITLES, api, errorText, gb, shortGpu } from "@/lib/api"
import { type ActionId, type Step, computeSteps, todo } from "@/lib/steps"
import { HelpPage } from "@/pages/help"
import { PlayPage } from "@/pages/play"
import { PreparePage } from "@/pages/prepare"
import { SettingsPage } from "@/pages/settings"

export type Tab = "play" | "prepare" | "settings" | "help"

const TITLES: Record<Tab, string> = { play: "Jouer", prepare: "Préparation", settings: "Réglages", help: "Aide" }

/** Everything the pages read and can do. */
export type Ctx = {
  snap: Snapshot
  facts: Facts | null
  steps: Step[]
  running: JobKind | null
  lines: string[]
  lastLine: string
  fps: string | null
  /** « Profil Series S · RTX 3050 · 32 Go » */
  profileLine: string
  captures: string[]
  captureNow: () => Promise<void>
  preparing: { done: number; total: number; current: string } | null
  go: (tab: Tab) => void
  act: (id: ActionId) => Promise<boolean>
  play: () => Promise<void>
  prepareAll: () => Promise<void>
  stop: () => void
  setMode: (mode: string) => Promise<void>
  pickDlss: () => Promise<void>
  open: (target: string) => void
  backup: () => Promise<void>
  refresh: () => Promise<Snapshot>
  refreshFacts: () => Promise<Facts | null>
}

const MAX_LINES = 1500

export default function App() {
  const [tab, setTab] = React.useState<Tab>("play")
  const [snap, setSnap] = React.useState<Snapshot | null>(null)
  const [facts, setFacts] = React.useState<Facts | null>(null)
  const [running, setRunning] = React.useState<JobKind | null>(null)
  const [lines, setLines] = React.useState<string[]>([])
  const [fps, setFps] = React.useState<string | null>(null)
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
      return f
    } catch (e) {
      toast.error(`Vérifications impossibles : ${errorText(e)}`)
      return null
    }
  }, [])

  const refreshCaptures = React.useCallback(() => {
    api.captures().then(setCaptures).catch(() => setCaptures([]))
  }, [])

  React.useEffect(() => {
    refresh().catch((e) => setLoadError(errorText(e)))
    refreshFacts()
    refreshCaptures()
    const subs = [
      listen<string>("capture", () => refreshCaptures()),
      listen<{ kind: JobKind; line: string }>("job-line", (e) => {
        const { kind, line } = e.payload
        if (kind === "play") {
          // « 9.8 fps, 102.0 ms/frame (...) » once a second: the number is enough.
          const m = line.match(/([\d.]+) fps, ([\d.]+) ms\/frame/)
          if (m) setFps(`${m[1].replace(".", ",")} images/s · ${Math.round(Number(m[2]))} ms par image`)
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
        setLines((prev) => [...prev.slice(-MAX_LINES + 1), `── ${JOB_TITLES[e.payload.kind]}`])
      }),
      listen<{ kind: JobKind; code: number }>("job-end", (e) => {
        setRunning(null)
        setLines((prev) => [...prev.slice(-MAX_LINES + 1), `── Terminé (code ${e.payload.code})`])
        refresh()
      }),
    ]
    return () => subs.forEach((p) => p.then((un) => un()))
  }, [refresh, refreshFacts, refreshCaptures])

  const steps = React.useMemo(() => (snap ? computeSteps(snap, facts) : []), [snap, facts])

  // ---------------------------------------------------------------- actions

  const runJob = async (kind: JobKind): Promise<boolean> => {
    try {
      const code = await api.runJob(kind)
      return code === 0
    } catch (e) {
      toast.error(errorText(e))
      return false
    }
  }

  const findGame = async (): Promise<boolean> => {
    const id = toast.loading("Recherche de game_scarlett_master.exe dans vos dossiers et sur vos disques…")
    let found: string[] = []
    try {
      found = await api.findGames()
    } finally {
      toast.dismiss(id)
    }
    let chosen: string | null = null
    if (found.length === 1) chosen = found[0]
    else if (found.length > 1) {
      chosen = await ask({
        title: `${found.length} copies du jeu trouvées`,
        body: <p>Laquelle utiliser ?</p>,
        choices: [
          ...found.slice(0, 4).map((f) => ({ label: f.replace(/\\game_scarlett_master\.exe$/i, ""), value: f, variant: "secondary" as const })),
          { label: "Annuler", value: "", variant: "ghost" as const },
        ],
      })
    }
    try {
      if (chosen) {
        await api.setGame(chosen)
        toast.success(`Jeu trouvé : ${chosen}`)
      } else {
        if (found.length === 0) toast.info("Le jeu n'a pas été trouvé automatiquement : indiquez le fichier game_scarlett_master.exe.")
        chosen = await api.pickGame()
      }
    } catch (e) {
      toast.error(errorText(e))
      chosen = null
    }
    await refresh()
    return !!chosen
  }

  const nvCache = async (): Promise<boolean> => {
    if (facts && !facts.nvidia) return true
    const opened = await api.nvidiaPanel()
    const r = await ask({
      title: "Cache de shaders NVIDIA",
      body: (
        <>
          <p>
            Par défaut, le pilote NVIDIA garde 4 Go de pipelines compilées par jeu. Une fois plein, chaque lancement
            recompile tout et paraît 1,5 à 2 fois plus lent.
          </p>
          <p>
            {opened
              ? "Le Panneau de configuration NVIDIA vient de s'ouvrir."
              : "Ouvrez le Panneau de configuration NVIDIA (clic droit sur le bureau, « Afficher plus d'options »)."}{" "}
            Dans « Gérer les paramètres 3D », onglet « Paramètres globaux », réglez « Taille du cache des shaders » sur
            « Illimitée », puis « Appliquer ».
          </p>
          <p>Le lanceur ne modifie pas le pilote lui-même.</p>
        </>
      ),
      choices: [
        { label: "C'est réglé sur Illimitée", value: "yes" },
        { label: "Pas encore", value: "no", variant: "ghost" },
      ],
    })
    if (r === null) return false
    await api.confirmNvCache(r === "yes")
    await refresh()
    return r === "yes"
  }

  const power = async (action: "high" | "revert"): Promise<boolean> => {
    try {
      toast.success(await api.power(action))
      await refreshFacts()
      await refresh()
      return true
    } catch (e) {
      toast.error(errorText(e))
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
      case "shaders":
      case "warm":
        return runJob(id)
      case "find_game":
        return findGame()
      case "pick_game":
        try {
          const p = await api.pickGame()
          await refresh()
          return !!p
        } catch (e) {
          toast.error(errorText(e))
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
      toast.success("Tout ce que le lanceur sait faire seul est prêt.")
      return
    }
    const r = await ask({
      title: "Tout préparer",
      body: (
        <>
          <p>Le lanceur va faire, dans l'ordre :</p>
          <ul className="flex list-none flex-col gap-1.5 pl-0">
            {auto.map((s) => (
              <li key={s.n} className="flex gap-2">
                <span className="w-4 font-bold text-white">{s.n}</span>
                <span>{s.action?.label ?? s.title}</span>
              </li>
            ))}
          </ul>
          <p>
            Les installations peuvent demander une autorisation de Windows. La traduction des shaders prend 15 à 30
            minutes ; le PC sera très occupé.
          </p>
        </>
      ),
      choices: [
        { label: "Commencer", value: "go" },
        { label: "Annuler", value: "", variant: "ghost" },
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
      const step = computeSteps(s, f).find((x) => x.auto === id)
      if (!step) continue
      setPreparing({ done, total, current: step.action?.label ?? step.title })
      const ok = await act(id)
      done++
      if (!ok) {
        setPreparing(null)
        toast.error(`Préparation interrompue à l'étape ${step.n} (${step.title}). Le détail est dans le journal.`)
        return
      }
    }
    setPreparing(null)
    await refresh()
    toast.success("Préparation terminée.")
    if (facts?.nvidia && !snap.settings.nvCacheOk) await nvCache()
  }

  const play = async () => {
    if (!snap) return
    if (snap.exeBuilt == null) {
      toast.warning("VIctory n'est pas encore compilé : voyez la préparation.")
      setTab("prepare")
      return
    }
    if (!snap.gameOk && !(await findGame())) return
    if (!snap.settings.shadersReady) {
      const r = await ask({
        title: "Shaders pas encore traduits",
        body: <p>Les premières minutes seront lentes (écrans noirs, à-coups). Jouer quand même ?</p>,
        choices: [
          { label: "Préparer d'abord", value: "prep" },
          { label: "Jouer quand même", value: "play", variant: "ghost" },
        ],
      })
      if (r !== "play") {
        if (r === "prep") setTab("prepare")
        return
      }
    }
    const ok = await runJob("play")
    await refresh()
    if (!ok) toast.info("La partie s'est terminée avec une erreur : ouvrez son journal.")
  }

  const profileLine = [
    "Profil Series S",
    facts ? (facts.gpuName ? shortGpu(facts.gpuName) : "carte inconnue") : "…",
    snap ? `${gb(snap.ramBytes)} Go` : "…",
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
    captures,
    captureNow: async () => {
      try {
        const f = await api.captureNow()
        toast.success(`Capture enregistrée : ${f.split(/[\\/]/).pop()}`)
        refreshCaptures()
      } catch (e) {
        toast.error(errorText(e))
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
        toast.error(errorText(e))
      }
      await refresh()
    },
    pickDlss: async () => {
      try {
        if (await api.pickDlss()) toast.success("Dossier du DLSS enregistré.")
      } catch (e) {
        toast.error(errorText(e))
      }
      await refresh()
    },
    open: (target) => api.open(target).catch((e) => toast.error(errorText(e))),
    backup: async () => {
      try {
        toast.success(`Sauvegardes copiées dans ${await api.backupSaves()}`)
      } catch (e) {
        toast.error(errorText(e))
      }
    },
    refresh,
    refreshFacts,
  }

  // Overall readiness for the dot next to the navigation.
  const pending = todo(steps).length
  const essentials = !!snap && snap.exeBuilt != null && snap.gameOk
  const readiness = !snap
    ? { color: "bg-muted-foreground", text: "Vérification…" }
    : !essentials
      ? { color: "bg-destructive", text: "Pas encore prêt : voir la préparation" }
      : pending > 0
        ? { color: "bg-warning", text: `${pending} étape${pending > 1 ? "s" : ""} de préparation à faire` }
        : { color: "bg-success", text: "Prêt à jouer" }

  // Profiles in the sidebar: the Series S settings, alone or with a DLSS mode.
  const profiles: Profile[] = snap
    ? [
        { id: "off", initials: "R", name: "Recommandé" },
        ...snap.dlssModes.map((m) => ({
          id: m.id,
          initials: m.id === "ultra" ? "UP" : m.id === "performance" ? "P" : "Q",
          name: m.nom,
          disabledWhy: snap.dlssOk ? undefined : "DLSS : choisissez d'abord le dossier de Streamline dans Réglages.",
        })),
      ]
    : []
  const selected =
    snap && snap.dlssOk && snap.dlssModes.some((m) => m.id === snap.settings.dlssMode) ? snap.settings.dlssMode : "off"

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
              { label: "Journal de la dernière partie", run: () => ctx.open("last_log"), disabled: !snap.lastLog },
              { label: "Dossier des journaux", run: () => ctx.open("logs") },
              { label: "Dossier des captures", run: () => ctx.open("captures") },
              { label: "Copier mes sauvegardes", run: ctx.backup },
            ]}
          />
        )}

        <Tabs value={tab} onValueChange={(v) => setTab(v as Tab)} className="relative min-w-0 flex-1 flex-col">
          <Backdrop files={captures} mode={tab === "play" ? "hero" : "dim"} />

          <header className="relative flex shrink-0 items-end justify-between gap-8 px-10 pt-6 pb-2">
            <div className="flex min-w-0 flex-col gap-1">
              <span className="truncate text-[12px] text-muted-foreground">VIctory S</span>
              <h1 className="text-[34px] leading-tight font-bold">{TITLES[tab]}</h1>
            </div>
            <div className="flex items-center gap-8 pb-1.5">
              <TabsList>
                <TabsTrigger value="play">Jouer</TabsTrigger>
                <TabsTrigger value="prepare">Préparation</TabsTrigger>
                <TabsTrigger value="settings">Réglages</TabsTrigger>
                <TabsTrigger value="help">Aide</TabsTrigger>
              </TabsList>
              <Tooltip>
                <TooltipTrigger asChild>
                  <button
                    type="button"
                    onClick={() => setTab("prepare")}
                    className="flex size-9 items-center justify-center rounded-full border border-border bg-[#2a2a2a] outline-none focus-visible:ring-2 focus-visible:ring-white/60"
                    aria-label={readiness.text}
                  >
                    <span className={`size-2.5 rounded-full ${readiness.color}`} />
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
              <span className="shrink-0 text-[12px] font-bold">{JOB_TITLES[running]}</span>
              <span className="min-w-0 flex-1 truncate font-mono text-[11.5px] text-muted-foreground">
                {lines[lines.length - 1]}
              </span>
              <Button variant="secondary" size="sm" onClick={() => api.stopJob()}>
                Arrêter
              </Button>
            </div>
          )}

          <footer className="relative flex shrink-0 items-center gap-6 border-t border-white/8 px-10 py-3.5 text-[11px] font-semibold tracking-[0.08em] text-[#8a8a8a]">
            <span>© VIctory S</span>
            <button type="button" className={link} onClick={() => ctx?.open("docs")}>
              Documentation
            </button>
            <button type="button" className={link} onClick={() => ctx?.open("license")}>
              Licence GPL-3.0
            </button>
            <button type="button" className={link} onClick={() => ctx?.open("issues")}>
              Signaler un problème
            </button>
          </footer>
        </Tabs>
      </div>
      {dialog}
      <Toaster position="bottom-right" />
    </TooltipProvider>
  )
}
