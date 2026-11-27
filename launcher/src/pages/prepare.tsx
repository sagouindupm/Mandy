import * as React from "react"

import type { Ctx } from "@/App"
import { StateBadge } from "@/components/state"
import { Button } from "@/components/ui/button"
import { Card, CardTitle } from "@/components/ui/card"
import { Progress } from "@/components/ui/progress"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { JOB_TITLES, gb } from "@/lib/api"
import type { Level, Step } from "@/lib/steps"

function StepCard({ step, ctx }: { step: Step; ctx: Ctx }) {
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
      {a.label}
    </Button>
  )
  return (
    <Card className="gap-0 p-5">
      <div className="flex items-center justify-between">
        <span className="label">Étape {step.n}</span>
        <StateBadge level={step.level} />
      </div>
      <CardTitle className="mt-3">{step.title}</CardTitle>
      <p className="mt-1.5 text-[13px] leading-relaxed text-muted-foreground">{step.explain}</p>
      <div className="mt-auto flex flex-col gap-4 pt-4">
        <div className="flex min-h-5 flex-wrap items-center gap-x-3 gap-y-1 text-[13px]">
          <span className="font-medium">{step.status}</span>
          {step.checks?.map((c) => (
            <span key={c.label} className={c.ok ? "text-success" : "text-destructive"}>
              {c.label} {c.ok ? "présent" : "absent"}
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
              {preparing ? preparing.current : `${ok} étapes prêtes sur ${steps.length}`}
            </CardTitle>
            <p className="text-[13px] text-muted-foreground">
              {preparing
                ? `Étape ${preparing.done + 1} sur ${preparing.total}.`
                : auto > 0
                  ? `${auto} étape${auto > 1 ? "s" : ""} automatique${auto > 1 ? "s" : ""}, dans l'ordre.`
                  : ok === steps.length
                    ? "Tout est prêt."
                    : "Les étapes restantes se règlent depuis leur carte."}
            </p>
          </div>
          {running && running !== "play" ? (
            <Button variant="secondary" onClick={ctx.stop}>
              Arrêter
            </Button>
          ) : (
            <Button onClick={ctx.prepareAll} disabled={running !== null || auto === 0}>
              Tout préparer
            </Button>
          )}
        </div>
        <Progress value={value} className="mt-5" />
        <div className="mt-4 flex items-center justify-between text-[12.5px]">
          <Button variant="link" className="text-[12.5px]" onClick={() => setShowLog((v) => !v)}>
            {showLog ? "Masquer le journal" : "Afficher le journal"}
          </Button>
          <span className="text-muted-foreground">
            {running ? JOB_TITLES[running] : ctx.lines.length > 0 ? "Dernière tâche terminée" : "Aucune tâche lancée"}
          </span>
        </div>
        {showLog && (
          <ScrollArea viewportRef={viewport} className="mt-3 h-52 rounded-md border border-border bg-field">
            <div className="px-4 py-3 font-mono text-[11.5px] leading-[1.6] text-white/70 select-text">
              {ctx.lines.length === 0 ? (
                <span className="text-white/40">Aucune ligne pour le moment.</span>
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
          <CardTitle>Ce PC</CardTitle>
          <Button variant="link" className="text-[12.5px]" onClick={() => ctx.refreshFacts()}>
            Revérifier
          </Button>
        </div>
        <div className="grid grid-cols-4 gap-8">
          <Fact
            label="Carte graphique"
            value={facts ? facts.gpuName || "Inconnue" : "…"}
            note={
              facts && facts.vram > 0
                ? `${gb(facts.vram)} Go de mémoire.${/RTX/i.test(facts.gpuName) ? " DLSS possible." : ""}`
                : "Mémoire inconnue."
            }
            level="info"
          />
          <Fact
            label="Mémoire vive"
            value={ram > 0 ? `${gb(ram)} Go` : "Inconnue"}
            note={
              lowRam
                ? `Moins de ${snap.lowRamBelowGb} Go : 3 Go de la mémoire du jeu passent dans la carte. Fermez le navigateur avant de jouer.`
                : `${snap.lowRamBelowGb} Go ou plus : rien à adapter.`
            }
            level={lowRam ? "warn" : "info"}
          />
          <Fact
            label="Mémoire virtuelle"
            value={!facts ? "…" : facts.autoPagefile == null ? "Inconnue" : facts.autoPagefile ? "Gérée par Windows" : "Taille fixe"}
            note="À laisser gérer par Windows : une petite taille fixe fait planter le jeu."
            level={facts?.autoPagefile === false ? "warn" : "info"}
            action={
              <Button variant="link" className="w-fit text-[12.5px]" onClick={() => ctx.open("perf_options")}>
                Options de performances
              </Button>
            }
          />
          <Fact
            label="Resizable BAR"
            value={!facts ? "…" : facts.bar1Mb >= 1024 ? "Actif" : facts.bar1Mb > 0 ? "Désactivé" : "Inconnu"}
            note={
              facts && facts.bar1Mb > 0
                ? `BAR1 : ${facts.bar1Mb} Mo.${facts.bar1Mb < 1024 ? " À activer dans le BIOS (Above 4G Decoding, Re-Size BAR)." : ""}`
                : "Se règle dans le BIOS (Above 4G Decoding, Re-Size BAR)."
            }
            level={facts && facts.bar1Mb > 0 && facts.bar1Mb < 1024 ? "warn" : "info"}
          />
        </div>
      </Card>
    </div>
  )
}
