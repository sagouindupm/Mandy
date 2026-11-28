import type { ReactNode } from "react"
import { MessageSquareHeart } from "lucide-react"

import type { Key } from "@/i18n/en"

import type { Ctx } from "@/App"
import { Button } from "@/components/ui/button"
import { Card, CardTitle } from "@/components/ui/card"
import { useI18n } from "@/i18n"

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <Card className="gap-3">
      <CardTitle>{title}</CardTitle>
      <div className="flex flex-col gap-3 text-[13.5px] leading-relaxed text-white/75">{children}</div>
    </Card>
  )
}

/** A key (written as on the keyboard, or a text to translate) and what it does. */
type Control = [string | Key, Key]

/** The keyboard and mouse layout (crates/victory-hle/src/modules/gameinput/keymap.rs, « controles » in
 * series-s/reglages.json): change them together. */
const ON_FOOT: Control[] = [
  ["W A S D", "help.controls.move"],
  ["help.controls.mouse", "help.controls.camera"],
  ["help.controls.rmb", "help.controls.aim"],
  ["help.controls.lmb", "help.controls.fire"],
  ["Shift", "help.controls.sprint"],
  ["Space", "help.controls.jump"],
  ["F", "help.controls.enter"],
  ["E", "help.controls.interact"],
  ["Q", "help.controls.cover"],
  ["R", "help.controls.reload"],
  ["Ctrl", "help.controls.crouch"],
  ["help.controls.mmb", "help.controls.behind"],
  ["X", "help.controls.walk"],
  ["Tab", "help.controls.weaponWheel"],
  ["help.controls.wheel", "help.controls.weapons"],
  ["M", "help.controls.map"],
  ["Esc", "help.controls.pause"],
  ["H", "help.controls.phone"],
  ["help.controls.arrows", "help.controls.dpad"],
  ["Enter / Backspace", "help.controls.menus"],
]

const DRIVING: Control[] = [
  ["W", "help.controls.accelerate"],
  ["S", "help.controls.brake"],
  ["A D", "help.controls.steer"],
  ["Space", "help.controls.handbrake"],
  ["help.controls.mouse", "help.controls.camera"],
  ["help.controls.rmb", "help.controls.driveAim"],
  ["help.controls.lmb", "help.controls.driveFire"],
  ["F", "help.controls.exit"],
  ["E", "help.controls.horn"],
  ["help.controls.mmb", "help.controls.behind"],
  ["R", "help.controls.cinematic"],
  ["V", "help.controls.toggle"],
]

function ControlTable({ title, rows }: { title: string; rows: Control[] }) {
  const { t } = useI18n()
  return (
    <div className="flex flex-col gap-2">
      <span className="font-bold text-white">{title}</span>
      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1">
        {rows.map(([key, what]) => (
          <div key={what} className="contents">
            <dt className="font-mono text-[12.5px] whitespace-nowrap text-white">
              {key.startsWith("help.") ? t(key as Key) : key}
            </dt>
            <dd>{t(what)}</dd>
          </div>
        ))}
      </dl>
    </div>
  )
}

export function HelpPage({ ctx }: { ctx: Ctx }) {
  const { t } = useI18n()
  return (
    <div className="grid grid-cols-2 gap-6">
      <Section title={t("help.order")}>
        <ol className="flex flex-col gap-2">
          {[
            ctx.snap.packaged ? t("help.order.packaged") : t("help.order.source"),
            t("help.order.nvcache"),
            t("help.order.play"),
            t("help.order.build"),
          ].map((text, i) => (
            <li key={i} className="flex gap-3">
              <span className="w-3 shrink-0 font-bold text-white">{i + 1}</span>
              <span>{text}</span>
            </li>
          ))}
        </ol>
      </Section>

      <Section title={t("help.what")}>
        <p>{t("help.what.p1")}</p>
        <p>{t("help.what.p2")}</p>
      </Section>

      <Section title={t("help.perf")}>
        <p>{t("help.perf.p1")}</p>
        <p>{t("help.perf.p2")}</p>
      </Section>

      <Section title={t("help.trouble")}>
        <p>{t("help.trouble.p1")}</p>
        <p>{t("help.trouble.p2")}</p>
        <div className="mt-1 flex items-center gap-4">
          <Button variant="secondary" size="sm" onClick={() => ctx.open("logs")}>
            {t("help.trouble.logs")}
          </Button>
          <Button variant="link" className="text-[12.5px]" onClick={() => ctx.open("issues")}>
            {t("footer.report")}
          </Button>
        </div>
      </Section>

      <Section title={t("help.vram")}>
        <p>{t("help.vram.p1")}</p>
        <ul className="flex list-disc flex-col gap-1.5 pl-5">
          <li>{t("help.vram.laptop")}</li>
          <li>{t("help.vram.close")}</li>
          <li>{t("help.vram.mode")}</li>
        </ul>
        <div className="mt-1 flex items-center gap-4">
          <Button variant="secondary" size="sm" onClick={() => ctx.go("settings")}>
            {t("nav.settings")}
          </Button>
        </div>
      </Section>

      <Section title={t("help.framegen")}>
        <p>{t("help.framegen.p1")}</p>
        <p>{t("help.framegen.p2")}</p>
        <p>{t("help.framegen.p3")}</p>
      </Section>

      <div className="col-span-2">
        <Section title={t("help.controls")}>
          <p>{t("help.controls.p1")}</p>
          <p>{t("help.controls.p2")}</p>
          <div className="grid grid-cols-2 gap-6">
            <ControlTable title={t("help.controls.foot")} rows={ON_FOOT} />
            <div className="flex flex-col gap-2">
              <ControlTable title={t("help.controls.driving")} rows={DRIVING} />
              <p className="text-white/60">{t("help.controls.others")}</p>
            </div>
          </div>
          <p>{t("help.controls.p3")}</p>
          <div className="mt-1 flex items-center gap-4">
            <Button variant="secondary" size="sm" onClick={() => ctx.go("settings")}>
              {t("settings.keyboard")}
            </Button>
          </div>
        </Section>
      </div>

      <div className="col-span-2">
        <Section title={t("help.log")}>
          <ol className="flex flex-col gap-2">
            {[t("help.log.s1"), t("help.log.s2"), t("help.log.s3")].map((text, i) => (
              <li key={i} className="flex gap-3">
                <span className="w-3 shrink-0 font-bold text-white">{i + 1}</span>
                <span>{text}</span>
              </li>
            ))}
          </ol>
          <div className="mt-1 flex items-center gap-4">
            <Button variant="secondary" size="sm" onClick={() => ctx.open("logs")}>
              {t("help.trouble.logs")}
            </Button>
            <Button variant="link" className="text-[12.5px]" onClick={() => ctx.open("last_log")} disabled={!ctx.snap.lastLog}>
              {t("shortcut.lastLog")}
            </Button>
          </div>
        </Section>
      </div>

      <div className="col-span-2">
        <Section title={t("footer.feedback")}>
          <p>{t("help.feedback.p")}</p>
          <div className="mt-1 flex items-center gap-4">
            <Button size="sm" onClick={() => ctx.open("feedback")}>
              <MessageSquareHeart strokeWidth={2} />
              {t("footer.feedback")}
            </Button>
          </div>
        </Section>
      </div>
    </div>
  )
}
