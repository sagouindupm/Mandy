import { Camera, Copy, Download, FolderOpen, Info, MessageSquareHeart, TriangleAlert } from "lucide-react"
import { toast } from "sonner"

import type { Ctx } from "@/App"
import { StateBadge } from "@/components/state"
import { Button } from "@/components/ui/button"
import { Card, CardDescription, CardTitle } from "@/components/ui/card"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Separator } from "@/components/ui/separator"
import { Switch } from "@/components/ui/switch"
import { LANGS, type Lang, useI18n } from "@/i18n"

function PathField({ value, empty }: { value: string; empty: string }) {
  return (
    <div
      className="flex h-10 items-center truncate rounded-md border border-input bg-field px-3.5 font-mono text-[12px] text-white/85 select-text"
      title={value}
    >
      <span className="truncate">{value || <span className="font-sans text-muted-foreground">{empty}</span>}</span>
    </div>
  )
}

/** « Detected profile »: what the launcher chose for this PC, and why (series-s/reglages.json, « materiel »). */
function ProfileCard({ ctx }: { ctx: Ctx }) {
  const { t, msg } = useI18n()
  const hw = ctx.snap.hardware
  if (!hw) return null
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(hw.text)
      toast.success(t("toast.profileCopied"))
    } catch {
      toast.error(t("toast.copyFailed"))
    }
  }
  return (
    <Card>
      <div className="flex items-start justify-between gap-6">
        <div className="flex min-w-0 flex-col gap-1">
          <CardTitle>{ctx.profileTitle}</CardTitle>
          <CardDescription>
            {t("settings.profile.desc", { pending: hw.pending ? t("settings.profile.pending") : "" })}
          </CardDescription>
        </div>
        <div className="flex shrink-0 items-center gap-4">
          <Button variant="link" className="text-[12.5px]" onClick={copy}>
            <Copy className="size-3.5" strokeWidth={2} />
            {t("common.copy")}
          </Button>
          <Button variant="secondary" size="sm" onClick={() => ctx.open("feedback")}>
            <MessageSquareHeart strokeWidth={2} />
            {t("footer.feedback")}
          </Button>
        </div>
      </div>
      <ul className="flex flex-col gap-1.5 text-[12.5px] leading-relaxed text-muted-foreground">
        {hw.reasons.map((r, k) => (
          <li key={`r${k}`} className="flex gap-2">
            <span className="text-white/40">•</span>
            <span>{msg(r)}</span>
          </li>
        ))}
        {hw.warnings.map((r, k) => (
          <li key={`w${k}`} className="flex items-center gap-2 text-warning">
            <TriangleAlert className="size-3.5 shrink-0" strokeWidth={2} />
            <span>{msg(r)}</span>
          </li>
        ))}
        {hw.tips.map((r, k) => (
          <li key={`t${k}`} className="flex items-center gap-2 text-white/80">
            <Info className="size-3.5 shrink-0" strokeWidth={2} />
            <span>{msg(r)}</span>
          </li>
        ))}
      </ul>
      <div className="flex flex-wrap items-center gap-1.5">
        <span className="label mr-1">{hw.env.length > 0 ? t("settings.profile.changed") : t("settings.profile.reference")}</span>
        {hw.env.map(([k, v]) => (
          <code key={k} className="rounded-sm border border-border bg-field px-2 py-1 font-mono text-[11px] text-white/80">
            {k.replace(/^VICTORY_/, "")}={v}
          </code>
        ))}
      </div>
    </Card>
  )
}

function EnvChips({ env }: { env: [string, string][] }) {
  return (
    <div className="flex flex-wrap gap-1.5">
      {env.map(([k, v]) => (
        <code key={k} className="rounded-sm border border-border bg-field px-2 py-1 font-mono text-[11px] text-white/80">
          {k.replace(/^VICTORY_/, "")}={v}
        </code>
      ))}
    </div>
  )
}

/** Frame generation (« framegen » in series-s/reglages.json): NVIDIA cards that have it only, off by default. */
function FramegenCard({ ctx }: { ctx: Ctx }) {
  const { t } = useI18n()
  const fg = ctx.snap.framegen
  // Not offered on AMD, Intel or older NVIDIA cards (nor before the graphics card is checked).
  if (!fg || fg.kind === "") return null
  const on = ctx.snap.settings.framegen
  const busy = ctx.running !== null
  const ready = fg.filesOk && (fg.kind !== "fsr3" || fg.fsr3Ready)
  return (
    <Card>
      <div className="flex items-start justify-between gap-6">
        <div className="flex min-w-0 flex-col gap-1">
          <div className="flex items-center gap-3">
            <CardTitle>{t("settings.framegen")}</CardTitle>
            <StateBadge
              level={!on ? "info" : ready ? "ok" : "warn"}
              label={!on ? t("settings.option.off") : ready ? t("settings.option.on") : t("settings.framegen.notReady")}
            />
          </div>
          <CardDescription>{fg.kind === "nvidia" ? t("settings.framegen.nvidia") : t("settings.framegen.fsr3")}</CardDescription>
        </div>
        <Switch
          checked={on}
          onCheckedChange={(v) => ctx.setOption("framegen", v)}
          disabled={busy}
          aria-label={t("settings.framegen")}
        />
      </div>
      <p className="flex gap-2 text-[12.5px] leading-relaxed text-warning">
        <TriangleAlert className="mt-0.5 size-3.5 shrink-0" strokeWidth={2} />
        <span>{t("settings.framegen.warning")}</span>
      </p>
      <div className="flex flex-col gap-2">
        <span className="label">{t("settings.framegen.files")}</span>
        <span className="text-[12.5px] text-muted-foreground">
          {fg.filesOk ? t("settings.framegen.filesOk") : t("settings.framegen.filesMissing", { files: fg.missing.join(", ") })}
        </span>
        {fg.kind === "fsr3" && !fg.fsr3Ready && <span className="text-[12.5px] text-warning">{t("settings.framegen.noFsr3")}</span>}
        <div className="mt-1 flex items-center gap-4">
          <Button variant="secondary" size="sm" onClick={ctx.getFramegenFiles} disabled={busy}>
            <Download strokeWidth={2} />
            {t("settings.framegen.get")}
          </Button>
          <Button variant="link" className="text-[12.5px]" onClick={() => ctx.open("framegen")}>
            {t("common.openFolder")}
          </Button>
        </div>
      </div>
      <EnvChips env={fg.env} />
    </Card>
  )
}

/** Low video memory mode (« petite_vram » in series-s/reglages.json): advised, never forced, on 6 GB cards. */
function LowVramCard({ ctx }: { ctx: Ctx }) {
  const { t } = useI18n()
  const on = ctx.snap.settings.petiteVram
  const hw = ctx.snap.hardware
  const advised = !!hw?.lowVramAdvised
  return (
    <Card>
      <div className="flex items-start justify-between gap-6">
        <div className="flex min-w-0 flex-col gap-1">
          <div className="flex items-center gap-3">
            <CardTitle>{t("settings.lowVram")}</CardTitle>
            <StateBadge
              level={on ? "ok" : advised ? "warn" : "info"}
              label={on ? t("settings.option.on") : advised ? t("settings.lowVram.recommended") : t("settings.option.off")}
            />
          </div>
          <CardDescription>{t("settings.lowVram.desc")}</CardDescription>
        </div>
        <Switch
          checked={on}
          onCheckedChange={(v) => ctx.setOption("petite_vram", v)}
          disabled={ctx.running === "play"}
          aria-label={t("settings.lowVram")}
        />
      </div>
      {advised && !on && (
        <p className="flex gap-2 text-[12.5px] leading-relaxed text-white/80">
          <Info className="mt-0.5 size-3.5 shrink-0" strokeWidth={2} />
          <span>{t("settings.lowVram.advised", { gb: hw?.vramGb ?? 0 })}</span>
        </p>
      )}
      <EnvChips env={ctx.snap.lowVramEnv} />
    </Card>
  )
}

/** Keys by their place on a US keyboard (AZERTY/QWERTZ automatic) or by their printed letter (« clavier_lettres » in
 * lanceur.json: VICTORY_KEYBOARD=letters, « controles » in series-s/reglages.json). */
function KeyboardCard({ ctx }: { ctx: Ctx }) {
  const { t } = useI18n()
  const letters = ctx.snap.settings.clavierLettres
  return (
    <Card>
      <div className="flex items-center justify-between gap-6">
        <div className="flex min-w-0 flex-col gap-1">
          <CardTitle>{t("settings.keyboard")}</CardTitle>
          <CardDescription>{t("settings.keyboard.desc")}</CardDescription>
        </div>
        <Select
          value={letters ? "letters" : "places"}
          onValueChange={(v) => ctx.setOption("clavier_lettres", v === "letters")}
          disabled={ctx.running === "play"}
        >
          <SelectTrigger className="w-[300px] shrink-0" aria-label={t("settings.keyboard")}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent position="popper">
            <SelectItem value="places">{t("settings.keyboard.places")}</SelectItem>
            <SelectItem value="letters">{t("settings.keyboard.letters")}</SelectItem>
          </SelectContent>
        </Select>
      </div>
    </Card>
  )
}

/** English or French, saved in lanceur.json (« langue ») and applied at once. */
function LanguageCard() {
  const { t, lang, setLang } = useI18n()
  return (
    <Card>
      <div className="flex items-center justify-between gap-6">
        <div className="flex min-w-0 flex-col gap-1">
          <CardTitle>{t("settings.language")}</CardTitle>
          <CardDescription>{t("settings.language.desc")}</CardDescription>
        </div>
        <Select value={lang} onValueChange={(v) => setLang(v as Lang)}>
          <SelectTrigger className="w-[200px] shrink-0" aria-label={t("settings.language")}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent position="popper">
            {LANGS.map((l) => (
              <SelectItem key={l.id} value={l.id} lang={l.id}>
                {l.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
    </Card>
  )
}

export function SettingsPage({ ctx }: { ctx: Ctx }) {
  const { t, data } = useI18n()
  const { snap } = ctx
  const st = snap.settings
  const modeKnown = snap.dlssModes.some((m) => m.id === st.dlssMode)
  const rtx = snap.hardware?.dlss !== false
  const mode = snap.dlssOk && rtx && modeKnown ? st.dlssMode : "off"
  const current = snap.dlssModes.find((m) => m.id === mode)
  const busy = ctx.running !== null
  const playing = ctx.running === "play"

  return (
    <div className="flex flex-col gap-6">
      <ProfileCard ctx={ctx} />
      <div className="grid grid-cols-2 gap-6">
        <Card>
          <div className="flex flex-col gap-1">
            <div className="flex items-center justify-between gap-4">
              <CardTitle>DLSS</CardTitle>
              <StateBadge
                level={snap.dlssOk && rtx ? "ok" : "info"}
                label={!rtx ? t("settings.dlss.unavailable") : snap.dlssOk ? t("level.ok") : t("settings.dlss.optional")}
              />
            </div>
            <CardDescription>{rtx ? t("settings.dlss.desc") : t("settings.dlss.descNoRtx")}</CardDescription>
          </div>
          <div className="flex flex-col gap-2">
            <span className="label">{t("settings.dlss.folder")}</span>
            <PathField value={st.dlssDir} empty={t("settings.dlss.noFolder")} />
            {snap.dlssMissing.length > 0 && (
              <p className="text-[12.5px] text-warning">{t("settings.dlss.missing", { files: snap.dlssMissing.join(", ") })}</p>
            )}
            <div className="mt-1 flex items-center gap-4">
              <Button variant="secondary" size="sm" onClick={ctx.pickDlss} disabled={busy || !rtx}>
                <FolderOpen strokeWidth={2} />
                {t("common.choose")}
              </Button>
              <Button variant="link" className="text-[12.5px]" onClick={() => ctx.open("streamline")}>
                <Download className="size-3.5" strokeWidth={2} />
                {t("settings.dlss.download")}
              </Button>
            </div>
          </div>
          <div className="flex flex-col gap-2">
            <span className="label">{t("settings.dlss.mode")}</span>
            <Select value={mode} onValueChange={ctx.setMode} disabled={busy}>
              <SelectTrigger className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent position="popper">
                <SelectItem value="off">{t("settings.dlss.off")}</SelectItem>
                {snap.dlssModes.map((m) => (
                  <SelectItem key={m.id} value={m.id} disabled={!snap.dlssOk || !rtx}>
                    {data(`dlss.${m.id}.nom`, m.nom)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-[12.5px] text-muted-foreground">
              {current ? data(`dlss.${current.id}.mesure`, current.mesure) : t("settings.dlss.notIncluded")}
            </p>
          </div>
        </Card>

        <Card>
          <div className="flex flex-col gap-1">
            <CardTitle>{t("settings.files")}</CardTitle>
            <CardDescription>{t("settings.files.desc")}</CardDescription>
          </div>
          <div className="flex flex-col gap-2">
            <span className="label">{t("settings.game")}</span>
            <PathField value={st.gameExe} empty={t("settings.game.notFound")} />
            <div className="mt-1 flex items-center gap-4">
              <Button variant="secondary" size="sm" onClick={() => ctx.act("pick_game")} disabled={busy}>
                <FolderOpen strokeWidth={2} />
                {t("common.change")}
              </Button>
              {snap.gameOk && (
                <Button variant="link" className="text-[12.5px]" onClick={() => ctx.open("game_dir")}>
                  {t("common.openFolder")}
                </Button>
              )}
            </div>
          </div>
          <Separator />
          <div className="flex flex-col gap-2">
            <span className="label">{t("settings.logs")}</span>
            <span className="text-[12.5px] text-muted-foreground">{t("settings.logs.desc")}</span>
            <div className="mt-1 flex items-center gap-4">
              <Button variant="secondary" size="sm" onClick={() => ctx.open("logs")}>
                <FolderOpen strokeWidth={2} />
                {t("common.openFolder")}
              </Button>
            </div>
          </div>
          <Separator />
          <div className="flex flex-col gap-2">
            <span className="label">{t("settings.captures")}</span>
            <span className="text-[12.5px] text-muted-foreground">{t("settings.captures.desc", { count: ctx.captures.length })}</span>
            <div className="mt-1 flex items-center gap-4">
              <Button variant="secondary" size="sm" onClick={ctx.captureNow} disabled={!playing}>
                <Camera strokeWidth={2} />
                {t("settings.captureNow")}
              </Button>
              <Button variant="link" className="text-[12.5px]" onClick={() => ctx.open("captures")}>
                {t("common.openFolder")}
              </Button>
            </div>
          </div>
          {snap.cacheInherited && <p className="text-[12.5px] text-muted-foreground">{t("settings.cache", { dir: snap.cacheDir })}</p>}
        </Card>
      </div>

      <div className="grid grid-cols-2 gap-6">
        <FramegenCard ctx={ctx} />
        <LowVramCard ctx={ctx} />
      </div>

      <KeyboardCard ctx={ctx} />

      <LanguageCard />

      <Card>
        <div className="flex flex-col gap-1">
          <CardTitle>{t("settings.applied")}</CardTitle>
          <CardDescription>{t("settings.applied.desc")}</CardDescription>
        </div>
        {snap.reglagesError && <p className="text-destructive">{snap.reglagesError}</p>}
        <div className="grid grid-cols-2 gap-x-8 gap-y-6">
          {snap.groups.map((g) => (
            <section key={g.id || g.titre} className="flex flex-col gap-2">
              <span className="label text-white">{data(`group.${g.id}.titre`, g.titre)}</span>
              <span className="text-[12.5px] leading-relaxed text-muted-foreground">{data(`group.${g.id}.note`, g.note)}</span>
              <div className="flex flex-wrap gap-1.5">
                {g.env.map(([k, v]) => {
                  const now = snap.applied.find(([ak]) => ak === k)?.[1]
                  const changed = now !== undefined && now !== v
                  return (
                    <code
                      key={k}
                      className={`rounded-sm border border-border bg-field px-2 py-1 font-mono text-[11px] ${now === undefined ? "text-white/35 line-through" : changed ? "border-[#FF3C78]/60 text-white" : "text-white/80"}`}
                      title={now === undefined ? t("settings.applied.removed") : changed ? t("settings.applied.changed", { value: v }) : undefined}
                    >
                      {k.replace(/^VICTORY_/, "")}={changed ? now : v}
                    </code>
                  )
                })}
              </div>
            </section>
          ))}
          <section className="flex flex-col gap-2">
            <span className="label text-white">{t("settings.rules")}</span>
            <ul className="flex flex-col gap-2 text-[12.5px] leading-relaxed text-muted-foreground">
              <li>
                <span className="text-white/85">{t("settings.rules.skipped")}</span>
                {t("settings.rules.skippedText", { count: snap.skipCount, file: snap.skipFile })}
              </li>
              <li>
                <span className="text-white/85">{t("settings.rules.lowRam", { gb: snap.lowRamBelowGb })}</span>
                {snap.lowRamEnv.map(([k, v]) => `${k}=${v}`).join(", ")}
                {snap.applied.some(([k]) => snap.lowRamEnv.some(([lk]) => lk === k))
                  ? t("settings.rules.appliedHere")
                  : t("settings.rules.notNeeded")}
              </li>
              <li>
                <span className="text-white/85">{t("settings.rules.dlss")}</span>
                {data("dlss.note", snap.dlssNote)}
              </li>
            </ul>
          </section>
        </div>
      </Card>
    </div>
  )
}
