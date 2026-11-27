import * as React from "react"
import { cn } from "cn"
import { Tabs as TabsPrimitive } from "radix-ui"

function Tabs({ className, ...props }: React.ComponentProps<typeof TabsPrimitive.Root>) {
  return <TabsPrimitive.Root data-slot="tabs" className={cn("flex", className)} {...props} />
}

// Top navigation: uppercase, letter-spaced text links; the active one white and bold over a 2px pink bar.
function TabsList({ className, ...props }: React.ComponentProps<typeof TabsPrimitive.List>) {
  return <TabsPrimitive.List data-slot="tabs-list" className={cn("flex items-center gap-8", className)} {...props} />
}

function TabsTrigger({ className, ...props }: React.ComponentProps<typeof TabsPrimitive.Trigger>) {
  return (
    <TabsPrimitive.Trigger
      data-slot="tabs-trigger"
      className={cn(
        "relative inline-flex h-10 items-center gap-2 text-[12.5px] font-semibold tracking-[0.12em] whitespace-nowrap text-white/65 uppercase transition-colors duration-200 outline-none hover:text-white focus-visible:text-white data-[state=active]:font-bold data-[state=active]:text-white",
        "after:absolute after:inset-x-0 after:bottom-0 after:h-0.5 after:bg-badge after:opacity-0 after:transition-opacity data-[state=active]:after:opacity-100",
        className
      )}
      {...props}
    />
  )
}

function TabsContent({ className, ...props }: React.ComponentProps<typeof TabsPrimitive.Content>) {
  return <TabsPrimitive.Content data-slot="tabs-content" className={cn("outline-none", className)} {...props} />
}

export { Tabs, TabsList, TabsTrigger, TabsContent }
