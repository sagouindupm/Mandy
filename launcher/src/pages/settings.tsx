import { Camera, Download, FolderOpen } from "lucide-react"

import type { Ctx } from "@/App"
import { StateBadge } from "@/components/state"
import { Button } from "@/components/ui/button"
import { Card, CardDescription, CardTitle } from "@/components/ui/card"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Separator } from "@/components/ui/separator"

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

export function SettingsPage({ ctx }: { ctx: Ctx }) {
  const { snap } = ctx
  const st = snap.settings
  const modeKnown = snap.dlssModes.some((m) => m.id === st.dlssMode)
  const mode = snap.dlssOk && modeKnown ? st.dlssMode : "off"
  const busy = ctx.running !== null
  const playing = ctx.running === "play"

  return (
    <div className="flex flex-col gap-6">
      <div className="grid grid-cols-2 gap-6">
        <Card>
          <div className="flex flex-col gap-1">
            <div className="flex items-center justify-between gap-4">
              <CardTitle>DLSS</CardTitle>
              <StateBadge level={snap.dlssOk ? "ok" : "info"} label={snap.dlssOk ? "Prêt" : "Facultatif"} />
            </div>
            <CardDescription>Cartes NVIDIA RTX, à la place de l'upscaler du jeu.</CardDescription>
          </div>
          <div className="flex flex-col gap-2">
            <span className="label">Dossier de Streamline (bin\x64)</span>
            <PathField value={st.dlssDir} empty="Aucun dossier" />
            {snap.dlssMissing.length > 0 && (
              <p className="text-[12.5px] text-warning">Il manque {snap.dlssMissing.join(", ")} : le jeu reviendra à son upscaler.</p>
            )}
            <div className="mt-1 flex items-center gap-4">
              <Button variant="secondary" size="sm" onClick={ctx.pickDlss} disabled={busy}>
                <FolderOpen strokeWidth={2} />
                Choisir
              </Button>
              <Button variant="link" className="text-[12.5px]" onClick={() => ctx.open("streamline")}>
                <Download className="size-3.5" strokeWidth={2} />
                Télécharger Streamline (NVIDIA)
              </Button>
            </div>
          </div>
          <div className="flex flex-col gap-2">
            <span className="label">Mode</span>
            <Select value={mode} onValueChange={ctx.setMode} disabled={busy}>
              <SelectTrigger className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent position="popper">
                <SelectItem value="off">Désactivé · 540p et upscaler du jeu</SelectItem>
                {snap.dlssModes.map((m) => (
                  <SelectItem key={m.id} value={m.id} disabled={!snap.dlssOk}>
                    {m.nom}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-[12.5px] text-muted-foreground">
              {snap.dlssModes.find((m) => m.id === mode)?.mesure ?? "Les DLL Streamline ne sont pas fournies (licence NVIDIA)."}
            </p>
          </div>
        </Card>

        <Card>
          <div className="flex flex-col gap-1">
            <CardTitle>Fichiers</CardTitle>
            <CardDescription>Votre copie du jeu, les journaux et les captures.</CardDescription>
          </div>
          <div className="flex flex-col gap-2">
            <span className="label">Jeu</span>
            <PathField value={st.gameExe} empty="Pas encore trouvé" />
            <div className="mt-1 flex items-center gap-4">
              <Button variant="secondary" size="sm" onClick={() => ctx.act("pick_game")} disabled={busy}>
                <FolderOpen strokeWidth={2} />
                Changer
              </Button>
              {snap.gameOk && (
                <Button variant="link" className="text-[12.5px]" onClick={() => ctx.open("game_dir")}>
                  Ouvrir le dossier
                </Button>
              )}
            </div>
          </div>
          <Separator />
          <div className="flex flex-col gap-2">
            <span className="label">Journaux</span>
            <span className="text-[12.5px] text-muted-foreground">Un fichier par tâche et par partie.</span>
            <div className="mt-1 flex items-center gap-4">
              <Button variant="secondary" size="sm" onClick={() => ctx.open("logs")}>
                <FolderOpen strokeWidth={2} />
                Ouvrir le dossier
              </Button>
            </div>
          </div>
          <Separator />
          <div className="flex flex-col gap-2">
            <span className="label">Captures</span>
            <span className="text-[12.5px] text-muted-foreground">
              {ctx.captures.length} image{ctx.captures.length > 1 ? "s" : ""}, en fond de l'accueil. Une capture est prise
              3 minutes après chaque lancement.
            </span>
            <div className="mt-1 flex items-center gap-4">
              <Button variant="secondary" size="sm" onClick={ctx.captureNow} disabled={!playing}>
                <Camera strokeWidth={2} />
                Capturer maintenant
              </Button>
              <Button variant="link" className="text-[12.5px]" onClick={() => ctx.open("captures")}>
                Ouvrir le dossier
              </Button>
            </div>
          </div>
          {snap.cacheInherited && (
            <p className="text-[12.5px] text-muted-foreground">Cache de VIctory (VICTORY_CACHE_DIR) : {snap.cacheDir}</p>
          )}
        </Card>
      </div>

      <Card>
        <div className="flex flex-col gap-1">
          <CardTitle>Réglages appliqués</CardTitle>
          <CardDescription>series-s\reglages.json, partagé avec le lanceur console. Lecture seule.</CardDescription>
        </div>
        {snap.reglagesError && <p className="text-destructive">{snap.reglagesError}</p>}
        <div className="grid grid-cols-2 gap-x-8 gap-y-6">
          {snap.groups.map((g) => (
            <section key={g.titre} className="flex flex-col gap-2">
              <span className="label text-white">{g.titre}</span>
              <span className="text-[12.5px] leading-relaxed text-muted-foreground">{g.note}</span>
              <div className="flex flex-wrap gap-1.5">
                {g.env.map(([k, v]) => {
                  const applied = snap.applied.some(([ak]) => ak === k)
                  return (
                    <code
                      key={k}
                      className={`rounded-sm border border-border bg-field px-2 py-1 font-mono text-[11px] ${applied ? "text-white/80" : "text-white/35 line-through"}`}
                      title={applied ? undefined : "Retiré : le DLSS est actif"}
                    >
                      {k.replace(/^VICTORY_/, "")}={v}
                    </code>
                  )
                })}
              </div>
            </section>
          ))}
          <section className="flex flex-col gap-2">
            <span className="label text-white">Règles</span>
            <ul className="flex flex-col gap-2 text-[12.5px] leading-relaxed text-muted-foreground">
              <li>
                <span className="text-white/85">Pipelines sautées : </span>
                {snap.skipCount} programmes ({snap.skipFile} et quelques passes coûteuses).
              </li>
              <li>
                <span className="text-white/85">Moins de {snap.lowRamBelowGb} Go de RAM : </span>
                {snap.lowRamEnv.map(([k, v]) => `${k}=${v}`).join(", ")}
                {snap.applied.some(([k]) => snap.lowRamEnv.some(([lk]) => lk === k)) ? ", appliqué ici." : ", pas nécessaire ici."}
              </li>
              <li>
                <span className="text-white/85">DLSS : </span>
                {snap.dlssNote}
              </li>
            </ul>
          </section>
        </div>
      </Card>
    </div>
  )
}
