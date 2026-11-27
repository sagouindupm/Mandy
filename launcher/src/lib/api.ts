// The calls to the Rust side (launcher/src-tauri/src/lib.rs) and the shapes it returns.
import { invoke } from "@tauri-apps/api/core"

export type Settings = {
  gameExe: string
  shadersDir: string
  shadersReady: boolean
  parties: number
  dlssDir: string
  dlssMode: string
  pipelinesParties: number
  pipelinesWhen: number | null
  nvCacheOk: boolean
  ancienPlan: string
}

export type DlssMode = { id: string; nom: string; mesure: string }
export type Group = { titre: string; note: string; env: [string, string][] }

/** The hardware profile of this PC (series-s/reglages.json, « materiel »; launcher/src-tauri/src/profile.rs). */
export type HardwareProfile = {
  label: string
  vendor: string
  vendorName: string
  reasons: string[]
  warnings: string[]
  tips: string[]
  env: [string, string][]
  dlss: boolean
  shadersNvidia: boolean
  /** The graphics card is not checked yet. */
  pending: boolean
  /** What the player copies into a feedback report. */
  text: string
}

export type Snapshot = {
  root: string
  exePath: string
  /** The downloaded package: victory.exe is included. */
  packaged: boolean
  version: string
  hardware: HardwareProfile | null
  exeBuilt: number | null
  reglagesError: string | null
  settings: Settings
  gameOk: boolean
  dlssOk: boolean
  dlssMissing: string[]
  ramBytes: number
  powerLine: "online" | "offline" | "unknown"
  lastLog: string | null
  logsDir: string
  profil: string
  groups: Group[]
  skipCount: number
  skipFile: string
  lowRamNote: string
  lowRamBelowGb: number
  lowRamEnv: [string, string][]
  dlssNote: string
  dlssModes: DlssMode[]
  applied: [string, string][]
  running: string | null
  warmDone: boolean
  cacheDir: string
  cacheInherited: boolean
}

export type Facts = {
  cargo: string
  winget: string
  vcTools: string
  winSdk: boolean
  autoPagefile: boolean | null
  gpuName: string
  nvidia: boolean
  vram: number
  cpuName: string
  threads: number
  avx2: boolean
  windows: string
  bar1Mb: number
  schemeGuid: string
  schemeName: string
  schemeGood: boolean
  battery: boolean
  dxCache: number
}

export type JobKind = "rust" | "vs" | "build" | "shaders" | "warm" | "play"

export const JOB_TITLES: Record<JobKind, string> = {
  rust: "Installation de Rust",
  vs: "Installation des outils C++",
  build: "Compilation de VIctory",
  shaders: "Traduction des shaders",
  warm: "Construction des pipelines",
  play: "Partie en cours",
}

export const api = {
  snapshot: () => invoke<Snapshot>("snapshot"),
  facts: () => invoke<Facts>("facts"),
  findGames: () => invoke<string[]>("find_games"),
  setGame: (path: string) => invoke<void>("set_game", { path }),
  pickGame: () => invoke<string | null>("pick_game"),
  pickDlss: () => invoke<string | null>("pick_dlss"),
  setMode: (mode: string) => invoke<void>("set_mode", { mode }),
  runJob: (kind: JobKind) => invoke<number>("run_job", { kind }),
  stopJob: () => invoke<void>("stop_job"),
  power: (action: "high" | "revert") => invoke<string>("power", { action }),
  nvidiaPanel: () => invoke<boolean>("nvidia_panel"),
  confirmNvCache: (ok: boolean) => invoke<void>("confirm_nv_cache", { ok }),
  open: (target: string) => invoke<void>("open", { target }),
  backupSaves: () => invoke<string>("backup_saves"),
  captures: () => invoke<string[]>("captures"),
  captureNow: () => invoke<string>("capture_now"),
  previewMode: () => invoke<boolean>("preview_mode"),
  previewShot: (name: string) => invoke<string>("preview_shot", { name }),
  previewDone: () => invoke<void>("preview_done"),
}

export const GB = 1024 ** 3

export function gb(bytes: number, digits = 0): string {
  return (bytes / GB).toLocaleString("fr-FR", { maximumFractionDigits: digits, minimumFractionDigits: digits })
}

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e)
}

export function dateTime(ms: number): string {
  const d = new Date(ms)
  return `${d.toLocaleDateString("fr-FR")} à ${d.toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" })}`
}

/// "NVIDIA GeForce RTX 3050" -> "RTX 3050" for the header.
export function shortGpu(name: string): string {
  return name.replace(/^(NVIDIA|AMD|Intel\(R\))\s+/i, "").replace(/^GeForce\s+/i, "").replace(/^Radeon\s+/i, "Radeon ").trim()
}
