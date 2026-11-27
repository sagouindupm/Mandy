import * as React from "react"
import { cn } from "cn"
import { Switch as SwitchPrimitive } from "radix-ui"

// On/off options: a pill, white when on (the window's primary colour), its knob sliding across.
function Switch({ className, ...props }: React.ComponentProps<typeof SwitchPrimitive.Root>) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      className={cn(
        "peer inline-flex h-6 w-11 shrink-0 items-center rounded-full border border-border bg-field p-0.5 outline-none transition-colors duration-200 focus-visible:ring-2 focus-visible:ring-white/60 disabled:cursor-not-allowed disabled:opacity-40 data-[state=checked]:border-white data-[state=checked]:bg-white",
        className
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot="switch-thumb"
        className="pointer-events-none block size-[18px] rounded-full bg-white/70 transition-transform duration-200 data-[state=checked]:translate-x-5 data-[state=checked]:bg-black"
      />
    </SwitchPrimitive.Root>
  )
}

export { Switch }
