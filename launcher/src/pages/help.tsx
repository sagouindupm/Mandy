import type { ReactNode } from "react"
import { MessageSquareHeart } from "lucide-react"

import type { Ctx } from "@/App"
import { Button } from "@/components/ui/button"
import { Card, CardTitle } from "@/components/ui/card"

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <Card className="gap-3">
      <CardTitle>{title}</CardTitle>
      <div className="flex flex-col gap-3 text-[13.5px] leading-relaxed text-white/75">{children}</div>
    </Card>
  )
}

export function HelpPage({ ctx }: { ctx: Ctx }) {
  return (
    <div className="grid grid-cols-2 gap-6">
      <Section title="Dans quel ordre">
        <ol className="flex flex-col gap-2">
          {[
            ctx.snap.packaged
              ? "Préparation, « Tout préparer » : le jeu, puis les shaders (15 à 30 min, une fois). VIctory est déjà inclus."
              : "Préparation, « Tout préparer » : Rust et outils C++ si besoin, compilation, jeu, shaders (15 à 30 min, une fois).",
            "Cache de shaders NVIDIA sur « Illimitée » (étape 5).",
            "Une première partie : le jeu note ses pipelines.",
            "« Construire » les pipelines (étape 4) : plus d'à-coups ensuite.",
          ].map((t, i) => (
            <li key={i} className="flex gap-3">
              <span className="w-3 shrink-0 font-bold text-white">{i + 1}</span>
              <span>{t}</span>
            </li>
          ))}
        </ol>
      </Section>

      <Section title="Ce que fait le lanceur">
        <p>
          Il lance votre copie d'un jeu Xbox Series sur PC, profil Series S, avec les réglages mesurés sur une carte de
          8 Go, un processeur 6 cœurs et 32 Go de RAM, ajustés tout seuls à votre PC (Réglages, « Profil détecté »).
        </p>
        <p>
          Rien du jeu n'est fourni ni modifié. Les réglages viennent de series-s\reglages.json, comme pour le lanceur
          console Sagouindupm.bat.
        </p>
      </Section>

      <Section title="Performances">
        <p>
          Environ 10 images par seconde sur RTX 3050 dans la mission du bateau. La console partage une mémoire entre
          processeur et carte ; le PC en a deux, que VIctory synchronise sans cesse.
        </p>
        <p>Avant de jouer : fermez le navigateur et Discord, branchez le chargeur d'un portable.</p>
      </Section>

      <Section title="En cas de problème">
        <p>
          Chaque tâche écrit un journal (compilation.txt, shaders_traduction.txt, pipelines_warm.txt, partie_….log) : joignez
          le dernier à votre signalement.
        </p>
        <p>
          Écran noir au début : shaders pas traduits. Plantage 0x8007000E : mémoire insuffisante, fermez les autres
          programmes et laissez Windows gérer la mémoire virtuelle.
        </p>
        <div className="mt-1 flex items-center gap-4">
          <Button variant="secondary" size="sm" onClick={() => ctx.open("logs")}>
            Journaux
          </Button>
          <Button variant="link" className="text-[12.5px]" onClick={() => ctx.open("issues")}>
            Signaler un problème
          </Button>
        </div>
      </Section>

      <div className="col-span-2">
        <Section title="Donner mon avis">
          <p>
            Vos images par seconde, les saccades, ce qui s'affiche mal : chaque retour sur une autre machine nous aide à
            régler les profils. Le bouton ouvre un formulaire GitHub déjà rempli avec votre configuration et le profil
            détecté ; vous le relisez, ajoutez vos impressions et l'envoyez vous-même (compte GitHub gratuit).
          </p>
          <div className="mt-1 flex items-center gap-4">
            <Button size="sm" onClick={() => ctx.open("feedback")}>
              <MessageSquareHeart strokeWidth={2} />
              Donner mon avis
            </Button>
          </div>
        </Section>
      </div>
    </div>
  )
}
