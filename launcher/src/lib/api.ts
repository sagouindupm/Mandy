// The calls to the Rust side (launcher/src-tauri/src/lib.rs) and the shapes it returns.
import { invoke } from "@tauri-apps/api/core"

import type { Key } from "@/i18n/en"
import type { Msg } from "@/i18n"

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
  /** « nom:taille » of the shared list the last « warm --used » built, and of the one whose shaders are cached. */
  pipelinesListe: string
  pipelinesImport: string
  /** The window's language (« langue » in lanceur.json, "en" when absent). */
  langue: "en" | "fr"
  /** The options of Settings (« framegen », « petite_vram » in lanceur.json), off by default. */
  framegen: boolean
  petiteVram: boolean
}

/** Frame generation on this PC (« framegen » in series-s/reglages.json). */
export type Framegen = {
  /** "fsr3" (RTX 20/30, GTX 16: FSR 3 behind DLSS-G's calls), "nvidia" (RTX 40/50: NVIDIA's own) or "" (not offered). */
  kind: "fsr3" | "nvidia" | ""
  /** The frame generation folder and what it still lacks (NVIDIA's files, the "framegen" task fetches them). */
  dir: string
  filesOk: boolean
  missing: string[]
  /** dlssg-to-fsr3's files are in the package's fg folder. */
  fsr3Ready: boolean
  env: [string, string][]
}

/** The shared pipeline list (series-s/pipelines): built before the first game. */
export type SharedList = { file: string; mark: string; count: number | null }

/** nom, mesure, titre, note: French texts of reglages.json, shown through i18n's data() (English by id). */
export type DlssMode = { id: string; nom: string; mesure: string }
export type Group = { id: string; titre: string; note: string; env: [string, string][] }

/** The hardware profile of this PC (series-s/reglages.json, « materiel »; launcher/src-tauri/src/profile.rs). */
export type HardwareProfile = {
  /** In English, for feedback reports; the window builds its own from gpu, vramGb, threads and ramGb. */
  label: string
  /** The card's short name, empty when unknown; zero when unknown. */
  gpu: string
  vramGb: number
  threads: number
  ramGb: number
  vendor: string
  vendorName: string
  reasons: Msg[]
  warnings: Msg[]
  tips: Msg[]
  env: [string, string][]
  dlss: boolean
  shadersNvidia: boolean
  /** Frame generation on this card ("fsr3", "nvidia" or ""), and the low video memory mode advised for it. */
  framegen: string
  lowVramAdvised: boolean
  /** The graphics card is not checked yet. */
  pending: boolean
  /** What the player copies into a feedback report. */
  text: string
}

/** A copy of the game, checked against « jeu » in series-s/reglages.json. */
export type GameCheck = {
  path: string
  complete: boolean
  rpfCount: number
  sizeGb: number
  /** What is missing (null when complete). */
  problem: Msg | null
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
  /** The game is set and its copy complete. */
  gameOk: boolean
  gameCheck: GameCheck | null
  /** Shaders in the cache after the last translation (null: unknown), and the least a complete copy gives. */
  shadersCount: number | null
  shadersMin: number
  sharedList: SharedList | null
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
  framegen: Framegen | null
  lowVramEnv: [string, string][]
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

export type JobKind = "rust" | "vs" | "build" | "import" | "shaders" | "warm" | "framegen" | "play"

/** The tasks' titles (i18n keys). */
export const JOB_KEYS: Record<JobKind, Key> = {
  rust: "job.rust",
  vs: "job.vs",
  build: "job.build",
  import: "job.import",
  shaders: "job.shaders",
  warm: "job.warm",
  framegen: "job.framegen",
  play: "job.play",
}

export const api = {
  snapshot: () => invoke<Snapshot>("snapshot"),
  facts: () => invoke<Facts>("facts"),
  findGames: () => invoke<GameCheck[]>("find_games"),
  setGame: (path: string) => invoke<void>("set_game", { path }),
  /** title and filter: the file dialog's texts, in the window's language. */
  pickGame: (title: string, filter: string) => invoke<string | null>("pick_game", { title, filter }),
  pickDlss: (title: string) => invoke<string | null>("pick_dlss", { title }),
  setMode: (mode: string) => invoke<void>("set_mode", { mode }),
  setOption: (name: "framegen" | "petite_vram", on: boolean) => invoke<void>("set_option", { name, on }),
  setLang: (lang: "en" | "fr") => invoke<void>("set_lang", { lang }),
  runJob: (kind: JobKind) => invoke<number>("run_job", { kind }),
  stopJob: () => invoke<void>("stop_job"),
  power: (action: "high" | "revert") => invoke<Msg>("power", { action }),
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

/// "NVIDIA GeForce RTX 3050" -> "RTX 3050" for the header.
export function shortGpu(name: string): string {
  return name.replace(/^(NVIDIA|AMD|Intel\(R\))\s+/i, "").replace(/^GeForce\s+/i, "").replace(/^Radeon\s+/i, "Radeon ").trim()
}
