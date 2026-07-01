import { Palette } from "lucide-react";
import { type Theme, useTheme } from "@/components/theme-provider";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";

const BASE_THEMES: { key: Theme; label: string }[] = [
  { key: "light", label: "Light" },
  { key: "dark", label: "Dark" },
  { key: "system", label: "System" },
];

const NITRO_THEMES: { key: Theme; label: string }[] = [
  { key: "crimson-moon", label: "Crimson Moon" },
  { key: "sepia", label: "Sepia" },
  { key: "midnight-blurple", label: "Midnight Blurple" },
  { key: "forest", label: "Forest" },
  { key: "dusk", label: "Dusk" },
  { key: "citrus-sherbet", label: "Citrus Sherbet" },
];

export function ModeToggle() {
  const { setTheme } = useTheme();

  const renderItem = ({ key, label }: { key: Theme; label: string }) => (
    <DropdownMenuItem key={key} onClick={() => setTheme(key)}>
      <span
        aria-hidden="true"
        className={cn(
          "size-3.5 shrink-0 rounded-full ring-1 ring-foreground/25 ring-inset",
          `theme-swatch-${key}`,
        )}
      />
      {label}
    </DropdownMenuItem>
  );

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button variant="outline" size="icon">
            <Palette className="h-[1.2rem] w-[1.2rem]" />
            <span className="sr-only">Toggle theme</span>
          </Button>
        }
      />
      <DropdownMenuContent align="end">
        {BASE_THEMES.map(renderItem)}
        <DropdownMenuSeparator />
        {NITRO_THEMES.map(renderItem)}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
