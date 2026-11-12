import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex h-5 items-center gap-1 whitespace-nowrap rounded-md border px-1.5 font-mono text-[11px]! font-medium uppercase leading-none tracking-[0.04em]",
  {
    variants: {
      variant: {
        default: "border-zinc-700/60 bg-zinc-800/60 text-zinc-300",
        accent: "badge-accent",
        blue: "border-blue-500/20 bg-blue-500/10 text-blue-300",
        green: "border-emerald-500/20 bg-emerald-500/10 text-emerald-300",
        yellow: "border-amber-500/20 bg-amber-500/10 text-amber-300",
        red: "border-red-500/20 bg-red-500/10 text-red-300",
        purple: "border-violet-500/20 bg-violet-500/10 text-violet-300",
      },
    },
    defaultVariants: { variant: "default" },
  },
);

export interface BadgeProps
  extends React.HTMLAttributes<HTMLSpanElement>,
    VariantProps<typeof badgeVariants> {}

export function Badge({ className, variant, ...props }: BadgeProps) {
  return <span className={cn(badgeVariants({ variant }), className)} {...props} />;
}
