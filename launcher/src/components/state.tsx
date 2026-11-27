import { Badge } from "@/components/ui/badge"
import { type Level, LEVEL_LABEL } from "@/lib/steps"

const VARIANT = { ok: "success", warn: "warning", error: "error", info: "info", wait: "info" } as const

export function StateBadge({ level, label }: { level: Level; label?: string }) {
  return <Badge variant={VARIANT[level]}>{label ?? LEVEL_LABEL[level]}</Badge>
}
