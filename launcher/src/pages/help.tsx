import type { ReactNode } from "react"
import { MessageSquareHeart } from "lucide-react"

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
