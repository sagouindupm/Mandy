import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "cn"
import { Slot } from "radix-ui"

// Pill buttons, uppercase and bold: white primary, grey-outlined secondary, round grey icon buttons, amber links.
const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-2 rounded-full border border-transparent font-bold tracking-[0.08em] whitespace-nowrap uppercase transition-[background-color,border-color,color,opacity] duration-200 outline-none select-none focus-visible:ring-2 focus-visible:ring-ring/70 focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:pointer-events-none disabled:opacity-40 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground hover:bg-[#e6e6e6]",
        secondary: "border-[#8a8a8a] bg-transparent text-foreground hover:border-white",
        ghost: "bg-transparent text-foreground hover:bg-white/8",
        icon: "bg-secondary text-muted-foreground hover:bg-[#333333] hover:text-foreground",
        link: "h-auto! rounded-sm px-0! font-semibold tracking-normal text-link normal-case hover:underline underline-offset-4",
      },
      size: {
        default: "h-10 px-8 text-[13px]",
        sm: "h-8 px-5 text-[11.5px]",
        lg: "h-14 px-14 text-[15px]",
        icon: "size-9",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
)

function Button({
  className,
  variant = "default",
  size = "default",
  asChild = false,
  ...props
}: React.ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean
  }) {
  const Comp = asChild ? Slot.Root : "button"

  return (
    <Comp
      data-slot="button"
      data-variant={variant}
      data-size={size}
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  )
}

export { Button, buttonVariants }
