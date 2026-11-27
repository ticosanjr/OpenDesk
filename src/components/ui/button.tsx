import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-lg shrink-0 text-sm font-medium transition-[color,background-color,border-color,box-shadow] duration-150 outline-none focus-visible:ring-2 focus-visible:ring-[color-mix(in_srgb,var(--workspace-accent)_60%,transparent)] focus-visible:ring-offset-1 focus-visible:ring-offset-zinc-950 disabled:pointer-events-none disabled:opacity-50 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default:
          "accent-button text-white shadow-sm",
        secondary:
          "border border-zinc-700/60 bg-zinc-800 text-zinc-100 hover:bg-zinc-700",
        outline:
          "border border-zinc-700/70 bg-zinc-900/30 text-zinc-200 hover:border-zinc-600 hover:bg-zinc-800/60 hover:text-white",
        ghost: "text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-100",
        destructive:
          "border border-red-500/30 bg-red-500/10 text-red-300 hover:border-red-500/40 hover:bg-red-500/15",
      },
      size: {
        xs: "h-7 min-h-7 px-2.5 text-[11.5px]",
        sm: "h-8 min-h-8 px-3 text-[12.5px]",
        default: "h-9 min-h-9 px-3.5",
        lg: "h-10 min-h-10 px-4",
        icon: "h-8 min-h-8 w-8 min-w-8 p-0",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {}

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, ...props }, ref) => (
    <button
      ref={ref}
      className={cn(buttonVariants({ variant, size }), className)}
      {...props}
    />
  ),
);
Button.displayName = "Button";
