// The six preparation steps and their state, from what the Rust side reports, in the window's language.
import {
  Boxes, Cpu, Download, ExternalLink, FolderOpen, FolderSearch, Hammer, HardDrive, Layers, type LucideIcon,
  RefreshCw, Undo2, Zap,
} from "lucide-react"

import type { I18n, Key } from "@/i18n"
import { type Facts, type Snapshot, GB } from "./api"

export type Level = "ok" | "warn" | "error" | "info" | "wait"

/** What a step's button does (handled in App.tsx). */
export type ActionId =
  | "rust"
  | "vs"
  | "build"
  | "find_game"
  | "pick_game"
  | "shaders"
  | "warm"
  | "nv_cache"
  | "power_high"
  | "power_revert"

export type Step = {
  n: number
  id: "engine" | "game" | "shaders" | "pipelines" | "nvcache" | "power"
  title: string
  icon: LucideIcon
  explain: string
  level: Level
  status: string
  /** Small sub-checks shown in the card (engine: Rust, C++ tools). */
  checks?: { label: string; ok: boolean }[]
  action?: { label: string; id: ActionId; primary: boolean; disabled?: string; icon?: LucideIcon }
  /** « Prepare everything » can do it by itself. */
  auto?: ActionId
}

export const LEVEL_KEY: Record<Level, Key> = {
  ok: "level.ok",
  warn: "level.warn",
  error: "level.error",
  info: "level.info",
  wait: "level.wait",
}

export function computeSteps(s: Snapshot, f: Facts | null, i: I18n): Step[] {
  const { t } = i
  const st = s.settings
  const exe = s.exeBuilt != null
  const steps: Step[] = []

  // 1. Engine
  {
    const step: Step = {
      n: 1,
      id: "engine",
      title: t("step.engine.title"), icon: Cpu,
      explain: t("step.engine.explain"),
      level: "wait",
      status: t("level.wait"),
    }
    if (s.packaged) {
      // The downloaded package: victory.exe comes with it (engine\victory.exe), nothing to compile.
      step.explain = t("step.engine.packaged")
      step.level = exe ? "ok" : "error"
      step.status = exe ? t("step.engine.included", { version: s.version }) : t("step.engine.missing")
    } else if (exe) {
      step.level = "ok"
      step.status = t("step.engine.compiled", { date: i.dateTime(s.exeBuilt!) })
      if (f?.cargo && f.vcTools) step.action = { label: t("step.engine.recompile"), id: "build", icon: RefreshCw, primary: false }
    } else if (f) {
      step.checks = [
        { label: t("step.engine.rust"), ok: !!f.cargo },
        { label: t("step.engine.cppTools"), ok: !!f.vcTools && f.winSdk },
      ]
      step.level = "error"
      if (!f.cargo) {
        step.status = t("step.engine.noRust")
        step.action = { label: t("step.engine.installRust"), icon: Download, id: "rust", primary: true }
        step.auto = "rust"
      } else if (!f.vcTools || !f.winSdk) {
        step.status = f.vcTools ? t("step.engine.noSdk") : t("step.engine.noCpp")
        step.action = { label: t("step.engine.installTools"), icon: Download, id: "vs", primary: true }
        step.auto = "vs"
      } else {
        step.status = t("step.engine.notCompiled")
        step.action = { label: t("step.engine.compile"), icon: Hammer, id: "build", primary: true }
        step.auto = "build"
      }
    }
    steps.push(step)
  }

  // 2. Game folder
  {
    const folder = st.gameExe.split(/[\\/]/).slice(-2, -1)[0] ?? ""
    // The exe alone, or a few archives: what is missing (« jeu » in reglages.json).
    const incomplete = s.gameCheck && !s.gameCheck.complete ? s.gameCheck : null
    const title = t("step.game.title")
    steps.push(
      incomplete
        ? {
            n: 2, id: "game", title, icon: FolderSearch,
            explain: incomplete.problem ? i.msg(incomplete.problem) : "",
            level: "error", status: t("step.game.incomplete"),
            action: { label: t("step.game.findFull"), icon: FolderSearch, id: "find_game", primary: true }, auto: "find_game",
          }
        : s.gameOk
        ? {
            n: 2, id: "game", title, icon: FolderSearch,
            explain: t("step.game.explain"),
            level: "ok", status: folder, action: { label: t("common.change"), id: "pick_game", icon: FolderOpen, primary: false },
          }
        : {
            n: 2, id: "game", title, icon: FolderSearch,
            explain: t("step.game.explain"),
            level: "error", status: t("step.game.notFound"),
            action: { label: t("step.game.find"), icon: FolderSearch, id: "find_game", primary: true }, auto: "find_game",
          },
    )
  }

  // 3. Shaders
  {
    const why = !exe ? t("step.needEngine") : !s.gameOk ? t("step.needGame") : undefined
    // A complete copy gives ~25 000 shaders; a handful means an incomplete copy, not « Ready ».
    const count = s.shadersCount
    const few = count != null && count < s.shadersMin
    const title = t("step.shaders.title")
    steps.push(
      few
        ? {
            n: 3, id: "shaders", title, icon: Layers,
            explain: t("step.shaders.fewExplain"),
            level: "error", status: t("step.shaders.only", { count: i.num(count) }),
            action: { label: t("step.shaders.retranslate"), icon: RefreshCw, id: "shaders", primary: true, disabled: why }, auto: "shaders",
          }
        : st.shadersReady
        ? {
            n: 3, id: "shaders", title, icon: Layers,
            explain: t("step.shaders.explain"),
            level: "ok", status: count != null ? t("step.shaders.count", { count: i.num(count) }) : t("step.shaders.done"),
            action: { label: t("step.shaders.retranslate"), icon: RefreshCw, id: "shaders", primary: false, disabled: why },
          }
        : {
            n: 3, id: "shaders", title, icon: Layers,
            explain: t("step.shaders.explain"),
            level: "error", status: t("step.shaders.notYet"),
            action: { label: t("step.shaders.translate"), icon: Layers, id: "shaders", primary: true, disabled: why }, auto: "shaders",
          },
    )
  }

  // 4. Pipelines
  {
    const step: Step = {
      n: 4, id: "pipelines", title: t("step.pipelines.title"), icon: Boxes,
      explain: t("step.pipelines.explain"),
      level: "info", status: "",
    }
    const why = !exe ? t("step.needEngine") : undefined
    const list = s.sharedList
    const listBuilt = !!list && st.pipelinesListe === list.mark
    const build = { label: t("step.pipelines.build"), icon: Boxes, id: "warm" as const, primary: true }
    const rebuild = { label: t("step.pipelines.rebuild"), icon: RefreshCw, id: "warm" as const, primary: false, disabled: why }
    if (list) {
      const n = list.count != null ? t("step.pipelines.count", { count: i.num(list.count) }) : t("step.pipelines.some")
      step.explain = t("step.pipelines.listExplain", { n })
    }
    if (list && !listBuilt) {
      // Before the first game: the shared list (step 3 imports it before translating).
      step.level = "error"
      step.status = t("step.pipelines.notYet")
      const needShaders = why ?? (!st.shadersReady ? t("step.needShaders") : undefined)
      step.action = { ...build, disabled: needShaders }
      step.auto = "warm"
    } else if (st.parties <= 0 && !listBuilt) {
      step.level = "info"
      step.status = t("step.pipelines.playFirst")
    } else if (st.parties <= 0) {
      step.level = "ok"
      step.status = st.pipelinesWhen
        ? t("step.pipelines.listBuiltOn", { date: i.dateTime(st.pipelinesWhen * 1000) })
        : t("step.pipelines.listBuilt")
      step.action = rebuild
    } else if (st.pipelinesParties < 0) {
      step.level = "error"
      step.status = t("step.pipelines.notBuilt", { count: st.parties })
      step.action = { ...build, disabled: why }
      step.auto = "warm"
    } else if (st.pipelinesParties < st.parties) {
      step.level = "warn"
      step.status = t("step.pipelines.newSince")
      step.action = { ...build, disabled: why }
      step.auto = "warm"
    } else {
      step.level = "ok"
      step.status = st.pipelinesWhen
        ? t("step.pipelines.upToDateOn", { date: i.dateTime(st.pipelinesWhen * 1000) })
        : t("step.pipelines.upToDate")
      step.action = rebuild
    }
    steps.push(step)
  }

  // 5. NVIDIA shader cache
  {
    const step: Step = {
      n: 5, id: "nvcache", title: t("step.nvcache.title"), icon: HardDrive,
      explain: t("step.nvcache.explain"),
      level: "wait", status: t("level.wait"),
    }
    if (f) {
      const size = t("unit.gb", { n: i.gb(f.dxCache, 1) })
      const act = { label: t("step.nvcache.panel"), icon: ExternalLink, id: "nv_cache" as const, primary: false }
      if (!f.nvidia) {
        step.level = "info"
        step.status = t("step.nvcache.notNvidia")
      } else if (st.nvCacheOk) {
        step.level = "ok"
        step.status = t("step.nvcache.confirmed", { size })
        step.action = { ...act, label: t("step.nvcache.review") }
      } else if (f.dxCache > 4.5 * GB) {
        step.level = "ok"
        step.status = t("step.nvcache.lifted", { size })
        step.action = act
      } else if (f.dxCache >= 3.5 * GB) {
        step.level = "error"
        step.status = t("step.nvcache.full", { size })
        step.action = { ...act, primary: true }
      } else {
        step.level = "warn"
        step.status = t("step.nvcache.toSet", { size })
        step.action = { ...act, primary: true }
      }
    }
    steps.push(step)
  }

  // 6. Power plan
  {
    const step: Step = {
      n: 6, id: "power", title: t("step.power.title"), icon: Zap,
      explain: t("step.power.explain"),
      level: "wait", status: t("level.wait"),
    }
    if (s.powerLine === "offline") {
      step.level = "error"
      step.status = t("step.power.battery")
    } else if (f) {
      if (f.schemeGood) {
        step.level = "ok"
        step.status = f.battery ? t("step.power.highMains") : t("step.power.high")
        if (st.ancienPlan) step.action = { label: t("step.power.revert"), icon: Undo2, id: "power_revert", primary: false }
      } else {
        step.level = "warn"
        step.status = t("step.power.scheme", { name: schemeLabel(f.schemeGuid, f.schemeName, i) })
        step.action = { label: t("step.power.activate"), icon: Zap, id: "power_high", primary: true }
        step.auto = "power_high"
      }
    }
    steps.push(step)
  }
  return steps
}

/** Windows' power plans by GUID; another one keeps the name Windows gives it (in Windows' language). */
export function schemeLabel(guid: string, fallback: string, { t }: I18n): string {
  switch (guid) {
    case "381b4222-f694-41f0-9685-ff5bb260df2e": return t("scheme.balanced")
    case "a1841308-3541-4fab-bc81-f71556f20b4a": return t("scheme.saver")
    case "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c": return t("scheme.high")
    case "e9a42b02-d5df-448d-aa00-03f14749eb61": return t("scheme.ultimate")
    default: return fallback || t("scheme.unknown")
  }
}

/** Steps still to do (error or warn). */
export function todo(steps: Step[]): Step[] {
  return steps.filter((s) => s.level === "error" || s.level === "warn")
}
