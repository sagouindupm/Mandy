import * as React from "react"
import { ListChecks, RefreshCw, Square } from "lucide-react"

import type { Ctx } from "@/App"
import { StateBadge } from "@/components/state"
import { Button } from "@/components/ui/button"
import { Card, CardTitle } from "@/components/ui/card"
import { Progress } from "@/components/ui/progress"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useI18n } from "@/i18n"
import { JOB_KEYS } from "@/lib/api"
import type { Level, Step } from "@/lib/steps"

function StepCard({ step, ctx }: { step: Step; ctx: Ctx }) {
  const { t } = useI18n()
  const [busy, setBusy] = React.useState(false)
  const blocked = ctx.running !== null || ctx.preparing !== null
  const a = step.action
  const run = async () => {
    if (!a) return
    setBusy(true)
    try {
      await ctx.act(a.id)
    } finally {
      setBusy(false)
    }
  }
  const button = a && (
    <Button variant={a.primary ? "default" : "secondary"} size="sm" disabled={blocked || busy || !!a.disabled} onClick={run}>
      {a.icon && <a.icon strokeWidth={2} />}
      {a.label}
    </Button>
  )
  return (
    <Card className="gap-0 p-5">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <span className="flex size-8 items-center justify-center rounded-md bg-[#2a2a2a] text-muted-foreground">
            <step.icon className="size-[17px]" strokeWidth={1.75} />
          </span>
          <span className="label">{t("step.n", { n: step.n })}</span>
        </div>
        <StateBadge level={step.level} />
      </div>
      <CardTitle className="mt-3">{step.title}</CardTitle>
      <p className="mt-1.5 text-[13px] leading-relaxed text-muted-foreground">{step.explain}</p>
      <div className="mt-auto flex flex-col gap-4 pt-4">
        <div className="flex min-h-5 flex-wrap items-center gap-x-3 gap-y-1 text-[13px]">
          <span className="font-medium">{step.status}</span>
          {step.checks?.map((c) => (
            <span key={c.label} className={c.ok ? "text-success" : "text-destructive"}>
              {t(c.ok ? "check.present" : "check.absent", { label: c.label })}
            </span>
          ))}
        </div>
        <div className="flex h-8 items-center">
          {a?.disabled ? (
            <Tooltip>
              <TooltipTrigger asChild>
                <span>{button}</span>
              </TooltipTrigger>
              <TooltipContent>{a.disabled}</TooltipContent>
            </Tooltip>
          ) : (
            button
          )}
        </div>
      </div>
    </Card>
  )
}

function Fact({ label, value, note, level, action }: {
  label: string
  value: string
  note: string
  level: Level
  action?: React.ReactNode
}) {
  return (
    <div className="flex min-w-0 flex-col gap-1.5">
      <div className="flex h-6 items-center gap-2">
        <span className="label">{label}</span>
        {level === "warn" && <StateBadge level="warn" />}
      </div>
      <span className="text-[15px] leading-snug font-bold">{value}</span>
      <p className="text-[12.5px] leading-relaxed text-muted-foreground">{note}</p>
      {action}
    </div>
  )
}

export function PreparePage({ ctx }: { ctx: Ctx }) {
  const { t, gb } = useI18n()
  const { steps, facts, snap, preparing, running } = ctx
  const [showLog, setShowLog] = React.useState(false)
  const viewport = React.useRef<HTMLDivElement>(null)
  const ok = steps.filter((s) => s.level === "ok" || s.level === "info").length
  const auto = steps.filter((s) => s.auto).length
  const value = preparing ? (preparing.done / Math.max(1, preparing.total)) * 100 : (ok / steps.length) * 100

  React.useEffect(() => {
    if (running && running !== "play") setShowLog(true)
  }, [running])
  React.useEffect(() => {
    const v = viewport.current
    if (v) v.scrollTop = v.scrollHeight
  }, [ctx.lines, showLog])

  const ram = snap.ramBytes
  const lowRam = ram > 0 && ram < snap.lowRamBelowGb * 1024 ** 3

  return (
    <div className="flex flex-col gap-6">
      <Card className="gap-0 p-6">
        <div className="flex items-center gap-8">
          <div className="flex min-w-0 flex-1 flex-col gap-1">
            <CardTitle className="text-[18px]">
              {preparing ? preparing.current : t("prepare.readyCount", { ok, total: steps.length })}
            </CardTitle>
            <p className="text-[13px] text-muted-foreground">
              {preparing
                ? t("prepare.stepOf", { n: preparing.done + 1, total: preparing.total })
                : auto > 0
                  ? t("prepare.auto", { count: auto })
                  : ok === steps.length
                    ? t("prepare.allReady")
                    : t("prepare.rest")}
            </p>
          </div>
          {running && running !== "play" ? (
            <Button variant="secondary" onClick={ctx.stop}>
              <Square className="size-3.5 fill-current" />
              {t("common.stop")}
            </Button>
          ) : (
            <Button onClick={ctx.prepareAll} disabled={running !== null || auto === 0}>
              <ListChecks strokeWidth={2} />
              {t("prepareAll.title")}
            </Button>
          )}
        </div>
        <Progress value={value} className="mt-5" />
        <div className="mt-4 flex items-center justify-between text-[12.5px]">
          <Button variant="link" className="text-[12.5px]" onClick={() => setShowLog((v) => !v)}>
            {showLog ? t("prepare.hideLog") : t("prepare.showLog")}
          </Button>
          <span className="text-muted-foreground">
            {running ? t(JOB_KEYS[running]) : ctx.lines.length > 0 ? t("prepare.lastDone") : t("prepare.noTask")}
          </span>
        </div>
        {showLog && (
          <ScrollArea viewportRef={viewport} className="mt-3 h-52 rounded-md border border-border bg-field">
            <div className="px-4 py-3 font-mono text-[11.5px] leading-[1.6] text-white/70 select-text">
              {ctx.lines.length === 0 ? (
                <span className="text-white/40">{t("prepare.noLines")}</span>
              ) : (
                ctx.lines.map((l, i) => (
                  <div key={i} className={`break-all ${l.startsWith("──") ? "mt-1 font-bold text-white" : ""}`}>
                    {l}
                  </div>
                ))
              )}
            </div>
          </ScrollArea>
        )}
      </Card>

      <div className="grid grid-cols-3 gap-4">
        {steps.map((s) => (
          <StepCard key={s.id} step={s} ctx={ctx} />
        ))}
      </div>

      <Card className="p-6">
        <div className="flex items-center justify-between">
          <CardTitle>{t("pc.title")}</CardTitle>
          <Button variant="link" className="text-[12.5px]" onClick={() => ctx.refreshFacts()}>
            <RefreshCw className="size-3.5" strokeWidth={2} />
            {t("pc.recheck")}
          </Button>
        </div>
        <div className="grid grid-cols-4 gap-8">
          <Fact
            label={t("pc.gpu")}
            value={facts ? facts.gpuName || t("pc.gpu.unknown") : "…"}
            note={
              facts && facts.vram > 0
                ? t("pc.gpu.memory", { gb: gb(facts.vram) }) + (/RTX/i.test(facts.gpuName) ? t("pc.gpu.dlss") : "")
                : t("pc.gpu.memoryUnknown")
            }
            level="info"
          />
          <Fact
            label={t("pc.ram")}
            value={ram > 0 ? t("unit.gb", { n: gb(ram) }) : t("pc.ram.unknown")}
            note={lowRam ? t("pc.ram.low", { gb: snap.lowRamBelowGb }) : t("pc.ram.ok", { gb: snap.lowRamBelowGb })}
            level={lowRam ? "warn" : "info"}
          />
          <Fact
            label={t("pc.pagefile")}
            value={
              !facts
                ? "…"
                : facts.autoPagefile == null
                  ? t("pc.pagefile.unknown")
                  : facts.autoPagefile
                    ? t("pc.pagefile.auto")
                    : t("pc.pagefile.fixed")
            }
            note={t("pc.pagefile.note")}
            level={facts?.autoPagefile === false ? "warn" : "info"}
            action={
              <Button variant="link" className="w-fit text-[12.5px]" onClick={() => ctx.open("perf_options")}>
                {t("pc.pagefile.options")}
              </Button>
            }
          />
          <Fact
            label={t("pc.bar")}
            value={!facts ? "…" : facts.bar1Mb >= 1024 ? t("pc.bar.on") : facts.bar1Mb > 0 ? t("pc.bar.off") : t("pc.bar.unknown")}
            note={
              facts && facts.bar1Mb > 0
                ? t("pc.bar.size", { mb: facts.bar1Mb }) + (facts.bar1Mb < 1024 ? t("pc.bar.turnOn") : "")
                : t("pc.bar.bios")
            }
            level={facts && facts.bar1Mb > 0 && facts.bar1Mb < 1024 ? "warn" : "info"}
          />
        </div>
      </Card>
    </div>
  )
}
