import { Check, Info, LoaderCircle, TriangleAlert, X } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { type Level, LEVEL_LABEL } from "@/lib/steps"

const VARIANT = { ok: "success", warn: "warning", error: "error", info: "info", wait: "info" } as const
const ICON = { ok: Check, warn: TriangleAlert, error: X, info: Info, wait: LoaderCircle } as const

export function StateBadge({ level, label }: { level: Level; label?: string }) {
  const Icon = ICON[level]
  return (
    <Badge variant={VARIANT[level]}>
      <Icon strokeWidth={2.25} className={level === "wait" ? "animate-spin" : undefined} />
      {label ?? LEVEL_LABEL[level]}
    </Badge>
  )
}
