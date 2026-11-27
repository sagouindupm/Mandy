import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "cn"
import { Slot } from "radix-ui"

// State labels: a coloured icon and the word, on a dark chip.
const badgeVariants = cva(
  "inline-flex h-6 w-fit shrink-0 items-center gap-1.5 rounded-sm bg-[#2a2a2a] px-2 text-[11px] font-bold tracking-[0.06em] whitespace-nowrap uppercase [&>svg]:size-3.5 [&>svg]:shrink-0",
  {
    variants: {
      variant: {
        success: "text-success",
        warning: "text-warning",
        error: "text-destructive",
        info: "text-muted-foreground",
      },
    },
    defaultVariants: {
      variant: "info",
    },
  }
)

function Badge({
  className,
  variant = "info",
  asChild = false,
  ...props
}: React.ComponentProps<"span"> &
  VariantProps<typeof badgeVariants> & { asChild?: boolean }) {
  const Comp = asChild ? Slot.Root : "span"

  return (
    <Comp
      data-slot="badge"
      data-variant={variant}
      className={cn(badgeVariants({ variant }), className)}
      {...props}
    />
  )
}

export { Badge, badgeVariants }
