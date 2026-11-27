// The window's frame: the player's screenshots as a background, the sidebar, the footer.
import * as React from "react"
import { convertFileSrc } from "@tauri-apps/api/core"
import { PanelLeftClose, PanelLeftOpen } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { Snapshot } from "@/lib/api"

const ROTATE_MS = 9000

/**
 * The player's own screenshots (%LOCALAPPDATA%\Sagouindupm\captures), cross-faded one after another.
 * Nothing is bundled: without screenshots the page stays plain charcoal.
 * hero: darkened from the left for the text; dim: nearly hidden behind the other pages.
 */
export function Backdrop({ files, mode }: { files: string[]; mode: "hero" | "dim" }) {
  const [index, setIndex] = React.useState(0)
  const urls = React.useMemo(() => files.map((f) => convertFileSrc(f)), [files])
  React.useEffect(() => {
    if (urls.length < 2) return
    const t = window.setInterval(() => setIndex((i) => (i + 1) % urls.length), ROTATE_MS)
    return () => window.clearInterval(t)
  }, [urls.length])
  if (urls.length === 0) return null
  return (
    <div className="pointer-events-none absolute inset-0 overflow-hidden" aria-hidden>
      {urls.map((u, i) => (
        <img
          key={u}
          src={u}
          alt=""
          className="absolute inset-0 size-full scale-[1.03] object-cover blur-[1.5px] transition-opacity duration-[1800ms] ease-in-out"
          style={{ opacity: i === index % urls.length ? 1 : 0 }}
        />
      ))}
      {mode === "hero" ? (
        <>
          <div className="absolute inset-0 bg-[linear-gradient(90deg,rgb(0_0_0/0.86)_0%,rgb(0_0_0/0.6)_45%,rgb(0_0_0/0.25)_100%)]" />
          <div className="absolute inset-x-0 top-0 h-40 bg-[linear-gradient(180deg,rgb(0_0_0/0.55),transparent)]" />
          <div className="absolute inset-x-0 bottom-0 h-40 bg-[linear-gradient(0deg,rgb(0_0_0/0.7),transparent)]" />
        </>
      ) : (
        <div className="absolute inset-0 bg-background/[0.93]" />
      )}
    </div>
  )
}

export type Profile = { id: string; initials: string; name: string; disabledWhy?: string }

export function Sidebar(props: {
  snap: Snapshot
  profiles: Profile[]
  selected: string
  onSelect: (id: string) => void
  shortcuts: { label: string; run: () => void; disabled?: boolean }[]
  collapsed: boolean
  onCollapse: () => void
}) {
  const { collapsed } = props
  return (
    <aside className={`flex shrink-0 flex-col bg-sidebar transition-[width] duration-200 ${collapsed ? "w-[72px]" : "w-64"}`}>
      <div className="flex h-[88px] shrink-0 items-center gap-3 px-[18px]">
        <div className="flex size-9 shrink-0 items-center justify-center rounded-md bg-badge text-[12px] font-black tracking-[-0.02em] text-black">
          VS
        </div>
        {!collapsed && <span className="truncate text-[13px] font-bold">VIctory S</span>}
      </div>

      <nav className="scroll-thin flex min-h-0 flex-1 flex-col overflow-y-auto pb-4">
        {!collapsed && <span className="label px-5 pt-2 pb-2">Profil</span>}
        <div className="flex flex-col gap-1 px-2">
          {props.profiles.map((p) => {
            const active = p.id === props.selected
            const row = (
              <button
                key={p.id}
                type="button"
                disabled={!!p.disabledWhy}
                onClick={() => props.onSelect(p.id)}
                className={`flex h-14 w-full items-center gap-3 rounded-md px-2.5 text-left transition-colors duration-200 outline-none focus-visible:ring-2 focus-visible:ring-white/60 disabled:opacity-40 ${
                  active ? "bg-[#2a2a2a]" : "hover:bg-[#1c1c1c]"
                }`}
              >
                <span
                  className={`flex size-9 shrink-0 items-center justify-center rounded-sm text-[11px] font-black ${
                    active ? "bg-white text-black" : "bg-[#2a2a2a] text-muted-foreground"
                  }`}
                >
                  {p.initials}
                </span>
                {!collapsed && (
                  <span className="flex min-w-0 flex-col">
                    <span className="truncate text-[13px] font-semibold">Series S</span>
                    <span className="truncate text-[12px] text-muted-foreground">{p.name}</span>
                  </span>
                )}
              </button>
            )
            return p.disabledWhy || collapsed ? (
              <Tooltip key={p.id}>
                <TooltipTrigger asChild>
                  <span className="block">{row}</span>
                </TooltipTrigger>
                <TooltipContent side="right">{p.disabledWhy ?? `Series S · ${p.name}`}</TooltipContent>
              </Tooltip>
            ) : (
              row
            )
          })}
        </div>

        {!collapsed && (
          <>
            <span className="label px-5 pt-7 pb-2">Raccourcis</span>
            <div className="flex flex-col">
              {props.shortcuts.map((s) => (
                <button
                  key={s.label}
                  type="button"
                  disabled={s.disabled}
                  onClick={s.run}
                  className="h-9 truncate px-5 text-left text-[13px] text-muted-foreground transition-colors duration-200 outline-none hover:text-white focus-visible:text-white disabled:opacity-40"
                >
                  {s.label}
                </button>
              ))}
            </div>
          </>
        )}
      </nav>

      <div className={`flex shrink-0 px-[18px] pb-5 ${collapsed ? "justify-center px-0" : ""}`}>
        <Button variant="icon" size="icon" onClick={props.onCollapse} aria-label={collapsed ? "Déplier le menu" : "Replier le menu"}>
          {collapsed ? <PanelLeftOpen /> : <PanelLeftClose />}
        </Button>
      </div>
    </aside>
  )
}
