// The window's language: English by default, or French. « langue » in lanceur.json (%LOCALAPPDATA%\Sagouindupm),
// read from the snapshot and written by set_lang (launcher/src-tauri/src/lib.rs); a copy in localStorage only
// avoids a flash of the other language at start.
//
// t(key, params) gives a text of en.ts / fr.ts; msg(m) a message of the Rust side (launcher/src-tauri/src/msg.rs);
// err(e) a command's error, either such a message as JSON or plain text (the system's own errors).
import * as React from "react"

import { api } from "@/lib/api"
import { type Dict, type Entry, type Key, en, reglagesEn } from "./en"
import { fr } from "./fr"

export type Lang = "en" | "fr"
export const LANGS: { id: Lang; name: string }[] = [
  { id: "en", name: "English" },
  { id: "fr", name: "Français" },
]

const DICTS: Record<Lang, Dict> = { en, fr }
const LOCALES: Record<Lang, string> = { en: "en-US", fr: "fr-FR" }
const STORE = "victory-s-langue"

export type Params = Record<string, string | number>

/** A text of the Rust side: a key and its parameters (texts, messages, lists of messages). */
export type Msg = { key: string; params?: Record<string, MsgParam> }
export type MsgParam = string | Msg | Msg[]

export type I18n = {
  lang: Lang
  locale: string
  /** Changes the language now; `save` (default) also writes it in lanceur.json. */
  setLang: (lang: Lang, save?: boolean) => void
  t: (key: Key, params?: Params) => string
  /** A message of the Rust side. */
  msg: (m: Msg) => string
  /** A command's error, translated when it is a message. */
  err: (e: unknown) => string
  /** A text of series-s/reglages.json (French) in the window's language, by its key in reglagesEn. */
  data: (key: string, french: string) => string
  num: (n: number, digits?: number) => string
  /** Bytes as GB, without the unit. */
  gb: (bytes: number, digits?: number) => string
  /** « 27/11/2026 à 14:05 » / « 11/27/2026 at 02:05 PM ». */
  dateTime: (ms: number) => string
  /** A day (« 2026-11-27 ») as the language writes it. */
  date: (iso: string) => string
}

const GB = 1024 ** 3

function fill(text: string, params?: Params): string {
  // A missing parameter is an optional part left out (« {lowered} »).
  return text.replace(/\{(\w+)\}/g, (_, k: string) => (params && k in params ? String(params[k]) : ""))
}

function make(lang: Lang, setLang: I18n["setLang"]): I18n {
  const dict = DICTS[lang]
  const locale = LOCALES[lang]
  const plural = new Intl.PluralRules(locale)

  const pick = (e: Entry, params?: Params): string =>
    typeof e === "string" ? e : plural.select(Number(params?.count ?? 0)) === "one" ? e.one : e.other
  const t = (key: Key, params?: Params) => fill(pick(dict[key], params), params)
  const data = (key: string, french: string) => (lang === "fr" ? french : (reglagesEn[key] ?? french))

  const msg = (m: Msg): string => {
    const raw = m.params ?? {}
    // A text of reglages.json: French there, English from reglagesEn.
    if (typeof raw.text === "string") return data(m.key, raw.text)
    const params: Params = {}
    for (const [k, v] of Object.entries(raw)) {
      params[k] = typeof v === "string" ? v : Array.isArray(v) ? v.map(msg).join(dict["list.sep"]) : msg(v)
    }
    return m.key in dict ? t(m.key as Key, params) : m.key
  }

  const err = (e: unknown): string => {
    const raw = typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e)
    if (raw.startsWith('{"key":')) {
      try {
        const m = JSON.parse(raw) as Msg
        if (typeof m.key === "string") return msg(m)
      } catch {
        // Plain text after all.
      }
    }
    return raw
  }

  const num = (n: number, digits?: number) =>
    digits === undefined ? n.toLocaleString(locale) : n.toLocaleString(locale, { maximumFractionDigits: digits, minimumFractionDigits: digits })

  return {
    lang,
    locale,
    setLang,
    t,
    msg,
    err,
    data,
    num,
    gb: (bytes, digits = 0) => num(bytes / GB, digits),
    dateTime: (ms) => {
      const d = new Date(ms)
      return t("common.dateTime", {
        date: d.toLocaleDateString(locale),
        time: d.toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" }),
      })
    },
    date: (iso) => {
      const d = new Date(`${iso}T00:00:00`)
      return Number.isNaN(d.getTime()) ? iso : d.toLocaleDateString(locale)
    },
  }
}

function stored(): Lang {
  try {
    return localStorage.getItem(STORE) === "fr" ? "fr" : "en"
  } catch {
    return "en"
  }
}

const I18nContext = React.createContext<I18n | null>(null)

export function I18nProvider({ children }: { children: React.ReactNode }) {
  const [lang, setLangState] = React.useState<Lang>(stored)

  const setLang = React.useCallback((l: Lang, save = true) => {
    setLangState(l)
    if (save) api.setLang(l).catch((e) => console.error(e))
  }, [])

  React.useEffect(() => {
    document.documentElement.lang = lang
    try {
      localStorage.setItem(STORE, lang)
    } catch {
      // No storage: the language still comes from lanceur.json.
    }
  }, [lang])

  const value = React.useMemo(() => make(lang, setLang), [lang, setLang])
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>
}

export function useI18n(): I18n {
  const v = React.useContext(I18nContext)
  if (!v) throw new Error("useI18n outside I18nProvider")
  return v
}

export type { Key }
