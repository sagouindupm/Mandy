import * as React from "react"
import { ChevronLeft, ChevronRight, Info, Play, Square, TriangleAlert } from "lucide-react"

import type { Ctx } from "@/App"
import { Button } from "@/components/ui/button"
import { useI18n } from "@/i18n"
import { todo } from "@/lib/steps"
import news from "../../news.json"

/** launcher/news.json: a day (« 2026-11-27 ») and the item in each language. */
type News = { date: string; en: { title: string; text: string }; fr: { title: string; text: string } }

function NewsCard() {
  const { t, lang, date } = useI18n()
  const items = news as News[]
  const [i, setI] = React.useState(0)
  const item = items[i]
  if (!item) return null
  const text = item[lang]
  const nav = "flex size-7 items-center justify-center rounded-full bg-white/10 text-white/80 transition-colors duration-200 hover:bg-white/20 hover:text-white disabled:opacity-30"
  return (
    <section className="flex w-[380px] shrink-0 flex-col gap-3 rounded-xl border border-white/10 bg-black/70 p-5">
      <div className="flex items-center justify-between">
        <span className="label text-white">{t("news.title")}</span>
        <div className="flex items-center gap-2">
          <span className="text-[11px] text-muted-foreground tabular-nums">
            {i + 1} / {items.length}
          </span>
          <button type="button" className={nav} onClick={() => setI(i - 1)} disabled={i === 0} aria-label={t("news.prev")}>
            <ChevronLeft className="size-4" />
          </button>
          <button type="button" className={nav} onClick={() => setI(i + 1)} disabled={i === items.length - 1} aria-label={t("news.next")}>
            <ChevronRight className="size-4" />
          </button>
        </div>
      </div>
      <div className="flex min-h-[112px] flex-col gap-1.5">
        <span className="text-[12px] text-muted-foreground">{date(item.date)}</span>
        <h3 className="text-[16px] leading-snug font-bold">{text.title}</h3>
        <p className="text-[13px] leading-relaxed text-white/75">{text.text}</p>
      </div>
    </section>
  )
}

export function PlayPage({ ctx }: { ctx: Ctx }) {
  const { t, msg, data, num } = useI18n()
  const { snap, steps, running, fps } = ctx
  const st = snap.settings
  const left = todo(steps)
  const playing = running === "play"
  const busy = running !== null && !playing
  const mode = snap.dlssOk && snap.hardware?.dlss !== false ? snap.dlssModes.find((m) => m.id === st.dlssMode) : undefined
  const hw = snap.hardware

  return (
    <div className="flex flex-1 items-end justify-between gap-10">
      <div className="flex min-w-0 flex-col">
        <h2 className="font-wordmark text-[54px] leading-[0.98] font-bold tracking-[0.1em] uppercase">
          VIctory S
        </h2>
        <button
          type="button"
          onClick={() => ctx.go("settings")}
          className="mt-4 w-fit text-left text-[14px] text-muted-foreground transition-colors duration-200 hover:text-white"
          title={t("play.seeWhy")}
        >
          {ctx.profileLine}
        </button>
        {hw?.warnings.map((w, k) => (
          <p key={k} className="mt-1.5 flex items-center gap-2 text-[13px] text-warning">
            <TriangleAlert className="size-3.5 shrink-0" strokeWidth={2} />
            {msg(w)}
          </p>
        ))}
        {hw?.tips.map((w, k) => (
          <p key={k} className="mt-1.5 flex items-center gap-2 text-[13px] text-white/75">
            <Info className="size-3.5 shrink-0" strokeWidth={2} />
            {msg(w)}
          </p>
        ))}

        <div className="mt-8 flex items-center gap-5">
          {playing ? (
            <>
              <Button size="lg" variant="secondary" onClick={ctx.stop}>
                <Square className="size-4 fill-current" />
                {t("common.stop")}
              </Button>
              <span className="text-[14px] text-white/85">
                {fps ? t("play.fps", { fps: num(fps.fps, 1), ms: Math.round(fps.ms) }) : t("play.starting")}
              </span>
            </>
          ) : (
            <Button size="lg" onClick={ctx.play} disabled={busy}>
              <Play className="fill-current" strokeWidth={1.75} />
              {t("nav.play")}
            </Button>
          )}
        </div>

        <div className="mt-4 flex min-h-5 items-center gap-2 text-[13px]">
          {busy ? (
            <span className="text-muted-foreground">{t("play.preparing")}</span>
          ) : left.length === 0 ? (
            <>
              <span className="size-2 rounded-full bg-success" />
              <span className="text-white/85">{t("status.ready")}</span>
            </>
          ) : (
            <Button variant="link" className="text-[13px]" onClick={() => ctx.go("prepare")}>
              {t("play.stepsLeft", { count: left.length })}
            </Button>
          )}
        </div>
        <p className="mt-2 text-[12px] text-muted-foreground">
          {mode ? data(`dlss.${mode.id}.nom`, mode.nom) : t("play.recommended")}
          {" · "}
          {st.parties > 0 ? t("play.games", { count: st.parties }) : t("play.noGames")}
        </p>
      </div>

      <NewsCard />
    </div>
  )
}
