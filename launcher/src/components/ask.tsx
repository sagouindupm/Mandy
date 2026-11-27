// A question in a dialog, awaited like a function: const v = await ask({ title, body, choices }).
import * as React from "react"

import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"

export type Choice = { label: string; value: string; variant?: "default" | "secondary" | "ghost" }
export type Question = { title: string; body: React.ReactNode; choices: Choice[] }

type Pending = Question & { resolve: (v: string | null) => void }

export function useAsk() {
  const [pending, setPending] = React.useState<Pending | null>(null)

  const ask = React.useCallback(
    (q: Question) => new Promise<string | null>((resolve) => setPending({ ...q, resolve })),
    [],
  )

  const answer = (v: string | null) => {
    pending?.resolve(v)
    setPending(null)
  }

  const dialog = (
    <Dialog open={!!pending} onOpenChange={(open) => !open && answer(null)}>
      <DialogContent showCloseButton={false} className="sm:max-w-[520px]">
        <DialogHeader className="gap-3">
          <DialogTitle>{pending?.title}</DialogTitle>
          <DialogDescription asChild>
            <div className="flex flex-col gap-3 text-[13.5px] leading-relaxed text-white/75">{pending?.body}</div>
          </DialogDescription>
        </DialogHeader>
        <DialogFooter className="gap-2">
          {pending?.choices.map((c) => (
            <Button key={c.value} variant={c.variant ?? "default"} onClick={() => answer(c.value)}>
              {c.label}
            </Button>
          ))}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )

  return { ask, dialog }
}
