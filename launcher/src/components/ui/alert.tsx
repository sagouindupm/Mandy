import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "cn"

// Summary panel with a state-coloured left edge.
const alertVariants = cva("relative flex w-full flex-col gap-1 rounded-lg border border-border border-l-[3px] bg-card px-5 py-4 text-left", {
  variants: {
    variant: {
      default: "border-l-muted-foreground",
      success: "border-l-success",
      warning: "border-l-warning",
      error: "border-l-destructive",
    },
  },
  defaultVariants: {
    variant: "default",
  },
})

function Alert({ className, variant, ...props }: React.ComponentProps<"div"> & VariantProps<typeof alertVariants>) {
  return <div data-slot="alert" role="status" className={cn(alertVariants({ variant }), className)} {...props} />
}

function AlertTitle({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="alert-title" className={cn("text-[14px] font-bold", className)} {...props} />
}

function AlertDescription({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="alert-description" className={cn("text-[13px] leading-relaxed text-muted-foreground", className)} {...props} />
}

export { Alert, AlertTitle, AlertDescription }
