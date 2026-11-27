// The six preparation steps and their state, from what the Rust side reports.
import { type Facts, type Snapshot, GB, dateTime, gb } from "./api"

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
  explain: string
  level: Level
  status: string
  /** Small sub-checks shown in the card (engine: Rust, C++ tools). */
  checks?: { label: string; ok: boolean }[]
  action?: { label: string; id: ActionId; primary: boolean; disabled?: string }
  /** « Tout préparer » can do it by itself. */
  auto?: ActionId
}

export const LEVEL_LABEL: Record<Level, string> = {
  ok: "Prêt",
  warn: "À vérifier",
  error: "À faire",
  info: "Info",
  wait: "Vérification…",
}

export function computeSteps(s: Snapshot, f: Facts | null): Step[] {
  const st = s.settings
  const exe = s.exeBuilt != null
  const steps: Step[] = []

  // 1. Engine
  {
    const step: Step = {
      n: 1,
      id: "engine",
      title: "Moteur VIctory",
      explain: "Compilé une fois sur ce PC avec Rust et les outils C++ de Microsoft (10 à 20 min).",
      level: "wait",
      status: "Vérification…",
    }
    if (f && !exe) step.checks = [
      { label: "Rust", ok: !!f.cargo },
      { label: "Outils C++", ok: !!f.vcTools && f.winSdk },
    ]
    if (exe) {
      step.level = "ok"
      step.status = `Compilé le ${dateTime(s.exeBuilt!)}`
      if (f?.cargo && f.vcTools) step.action = { label: "Recompiler", id: "build", primary: false }
    } else if (f) {
      step.level = "error"
      if (!f.cargo) {
        step.status = "Rust n'est pas installé"
        step.action = { label: "Installer Rust", id: "rust", primary: true }
        step.auto = "rust"
      } else if (!f.vcTools || !f.winSdk) {
        step.status = f.vcTools ? "SDK Windows absent" : "Outils C++ de Microsoft absents"
        step.action = { label: "Installer les outils", id: "vs", primary: true }
        step.auto = "vs"
      } else {
        step.status = "Pas encore compilé"
        step.action = { label: "Compiler", id: "build", primary: true }
        step.auto = "build"
      }
    }
    steps.push(step)
  }

  // 2. Game folder
  {
    const folder = st.gameExe.split(/[\\/]/).slice(-2, -1)[0] ?? ""
    steps.push(
      s.gameOk
        ? {
            n: 2, id: "game", title: "Dossier du jeu",
            explain: "Votre copie du jeu : le fichier game_scarlett_master.exe.",
            level: "ok", status: folder, action: { label: "Changer", id: "pick_game", primary: false },
          }
        : {
            n: 2, id: "game", title: "Dossier du jeu",
            explain: "Votre copie du jeu : le fichier game_scarlett_master.exe.",
            level: "error", status: "Jeu pas encore trouvé",
            action: { label: "Chercher le jeu", id: "find_game", primary: true }, auto: "find_game",
          },
    )
  }

  // 3. Shaders
  {
    const why = !exe ? "Il faut d'abord le moteur (étape 1)." : !s.gameOk ? "Il faut d'abord le jeu (étape 2)." : undefined
    steps.push(
      st.shadersReady
        ? {
            n: 3, id: "shaders", title: "Shaders traduits",
            explain: "Traduits une fois pour votre carte (15 à 30 min) : moins d'écrans noirs et d'à-coups.",
            level: "ok", status: "Traduits", action: { label: "Retraduire", id: "shaders", primary: false, disabled: why },
          }
        : {
            n: 3, id: "shaders", title: "Shaders traduits",
            explain: "Traduits une fois pour votre carte (15 à 30 min) : moins d'écrans noirs et d'à-coups.",
            level: "error", status: "Pas encore traduits",
            action: { label: "Traduire", id: "shaders", primary: true, disabled: why }, auto: "shaders",
          },
    )
  }

  // 4. Pipelines
  {
    const step: Step = {
      n: 4, id: "pipelines", title: "Pipelines construites",
      explain: "Chaque partie note les pipelines du jeu ; les construire hors jeu (quelques minutes) retire les à-coups.",
      level: "info", status: "",
    }
    const why = !exe ? "Il faut d'abord le moteur (étape 1)." : undefined
    if (st.parties <= 0) {
      step.level = "info"
      step.status = "Jouez d'abord une partie"
    } else if (st.pipelinesParties < 0) {
      step.level = "error"
      step.status = `Pas construites · ${st.parties} partie${st.parties > 1 ? "s" : ""} jouée${st.parties > 1 ? "s" : ""}`
      step.action = { label: "Construire", id: "warm", primary: true, disabled: why }
      step.auto = "warm"
    } else if (st.pipelinesParties < st.parties) {
      step.level = "warn"
      step.status = "Nouvelles pipelines notées depuis"
      step.action = { label: "Construire", id: "warm", primary: true, disabled: why }
      step.auto = "warm"
    } else {
      step.level = "ok"
      step.status = st.pipelinesWhen ? `À jour · ${dateTime(st.pipelinesWhen * 1000)}` : "À jour"
      step.action = { label: "Reconstruire", id: "warm", primary: false, disabled: why }
    }
    steps.push(step)
  }

  // 5. NVIDIA shader cache
  {
    const step: Step = {
      n: 5, id: "nvcache", title: "Cache NVIDIA illimité",
      explain: "Le pilote garde 4 Go de pipelines par jeu ; plein, il recompile tout à chaque lancement.",
      level: "wait", status: "Vérification…",
    }
    if (f) {
      const size = `${gb(f.dxCache, 1)} Go`
      const act = { label: "Panneau NVIDIA", id: "nv_cache" as const, primary: false }
      if (!f.nvidia) {
        step.level = "info"
        step.status = "Non concerné : carte non NVIDIA"
      } else if (st.nvCacheOk) {
        step.level = "ok"
        step.status = `Illimité (confirmé) · ${size}`
        step.action = { ...act, label: "Revoir" }
      } else if (f.dxCache > 4.5 * GB) {
        step.level = "ok"
        step.status = `${size} : limite levée`
        step.action = act
      } else if (f.dxCache >= 3.5 * GB) {
        step.level = "error"
        step.status = `${size} : sans doute plein`
        step.action = { ...act, primary: true }
      } else {
        step.level = "warn"
        step.status = `${size} · à régler sur « Illimitée »`
        step.action = { ...act, primary: true }
      }
    }
    steps.push(step)
  }

  // 6. Power plan
  {
    const step: Step = {
      n: 6, id: "power", title: "Alimentation",
      explain: "Mode « Performances élevées » de Windows : le processeur et la carte ne ralentissent pas. Réversible.",
      level: "wait", status: "Vérification…",
    }
    if (s.powerLine === "offline") {
      step.level = "error"
      step.status = "Sur batterie : branchez le chargeur"
    } else if (f) {
      if (f.schemeGood) {
        step.level = "ok"
        step.status = f.battery ? "Performances élevées, sur secteur" : "Performances élevées"
        if (st.ancienPlan) step.action = { label: "Revenir à l'ancien mode", id: "power_revert", primary: false }
      } else {
        step.level = "warn"
        step.status = `Mode « ${schemeLabel(f.schemeGuid, f.schemeName)} »`
        step.action = { label: "Activer", id: "power_high", primary: true }
        step.auto = "power_high"
      }
    }
    steps.push(step)
  }
  return steps
}

export function schemeLabel(guid: string, fallback: string): string {
  switch (guid) {
    case "381b4222-f694-41f0-9685-ff5bb260df2e": return "Utilisation normale"
    case "a1841308-3541-4fab-bc81-f71556f20b4a": return "Économie d'énergie"
    case "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c": return "Performances élevées"
    case "e9a42b02-d5df-448d-aa00-03f14749eb61": return "Performances optimales"
    default: return fallback || "inconnu"
  }
}

/** Steps still to do (error or warn). */
export function todo(steps: Step[]): Step[] {
  return steps.filter((s) => s.level === "error" || s.level === "warn")
}
