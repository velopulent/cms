import { Palette } from "lucide-react";
import { type Theme, useTheme } from "@/components/theme-provider";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
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
  { key: "blurple-twilight", label: "Blurple Twilight" },
  { key: "forest", label: "Forest" },
  { key: "dusk", label: "Dusk" },
  { key: "aurora", label: "Aurora" },
  { key: "sunset", label: "Sunset" },
  { key: "mars", label: "Mars" },
  { key: "retro-storm", label: "Retro Storm" },
  { key: "under-the-sea", label: "Under the Sea" },
  { key: "strawberry-lemonade", label: "Strawberry Lemonade" },
  { key: "neon-nights", label: "Neon Nights" },
  { key: "citrus-sherbet", label: "Citrus Sherbet" },
  { key: "desert-khaki", label: "Desert Khaki" },
  { key: "sunrise", label: "Sunrise" },
  { key: "hanami", label: "Hanami" },
  { key: "cotton-candy", label: "Cotton Candy" },
  { key: "mint-apple", label: "Mint Apple" },
];

export function ModeToggle() {
  const { theme, setTheme } = useTheme();

  const renderItem = ({ key, label }: { key: Theme; label: string }) => {
    const active = theme === key;
    return (
      <DropdownMenuRadioItem
        key={key}
        value={key}
        className={cn(active && "bg-accent text-accent-foreground")}
      >
        <span
          aria-hidden="true"
          className={cn(
            "size-3.5 shrink-0 rounded-full ring-1 ring-foreground/25 ring-inset",
            `theme-swatch-${key}`,
          )}
        />
        {label}
      </DropdownMenuRadioItem>
    );
  };

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
      <DropdownMenuContent align="end" className={"w-fit"}>
        <DropdownMenuRadioGroup
          value={theme}
          onValueChange={(value) => setTheme(value as Theme)}
        >
          {BASE_THEMES.map(renderItem)}
          <DropdownMenuSeparator />
          <ScrollArea className="h-64 pr-1">
            {NITRO_THEMES.map(renderItem)}
          </ScrollArea>
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
