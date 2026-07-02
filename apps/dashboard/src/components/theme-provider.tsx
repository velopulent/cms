import { createContext, useContext, useEffect, useState } from "react";

export type Theme =
  | "dark"
  | "light"
  | "system"
  | "crimson-moon"
  | "sepia"
  | "midnight-blurple"
  | "blurple-twilight"
  | "forest"
  | "dusk"
  | "aurora"
  | "sunset"
  | "mars"
  | "retro-storm"
  | "under-the-sea"
  | "strawberry-lemonade"
  | "neon-nights"
  | "citrus-sherbet"
  | "desert-khaki"
  | "sunrise"
  | "hanami"
  | "cotton-candy"
  | "mint-apple";

// Custom palettes are applied to <html> as a `theme-<key>` class (e.g.
// `theme-sepia`) — the `theme-` prefix keeps them from colliding with Tailwind
// utility classes of the same name (notably `sepia`, `grayscale`, `invert`).
// Each derives its tokens from the shared `.theme-dark` / `.theme-light` marker,
// added alongside the palette class in the effect below.
const DARK_THEMES = [
  "crimson-moon",
  "sepia",
  "midnight-blurple",
  "blurple-twilight",
  "forest",
  "dusk",
  "aurora",
  "sunset",
  "mars",
  "retro-storm",
  "under-the-sea",
  "strawberry-lemonade",
  "neon-nights",
] as const;
const LIGHT_THEMES = [
  "citrus-sherbet",
  "desert-khaki",
  "sunrise",
  "hanami",
  "cotton-candy",
  "mint-apple",
] as const;
const CUSTOM_THEMES: readonly string[] = [...DARK_THEMES, ...LIGHT_THEMES];

type ThemeProviderProps = {
  children: React.ReactNode;
  defaultTheme?: Theme;
  storageKey?: string;
};

type ThemeProviderState = {
  theme: Theme;
  setTheme: (theme: Theme) => void;
};

const initialState: ThemeProviderState = {
  theme: "system",
  setTheme: () => null,
};

const ThemeProviderContext = createContext<ThemeProviderState>(initialState);

export function ThemeProvider({
  children,
  defaultTheme = "system",
  storageKey = "vite-ui-theme",
  ...props
}: ThemeProviderProps) {
  const [theme, setTheme] = useState<Theme>(
    () => (localStorage.getItem(storageKey) as Theme) || defaultTheme,
  );

  useEffect(() => {
    const root = window.document.documentElement;

    root.classList.remove(
      "light",
      "dark",
      "theme-dark",
      "theme-light",
      ...CUSTOM_THEMES.map((t) => `theme-${t}`),
    );

    if (theme === "system") {
      const systemTheme = window.matchMedia("(prefers-color-scheme: dark)")
        .matches
        ? "dark"
        : "light";

      root.classList.add(systemTheme);
      return;
    }

    if ((DARK_THEMES as readonly string[]).includes(theme)) {
      root.classList.add(`theme-${theme}`, "theme-dark");
      return;
    }

    if ((LIGHT_THEMES as readonly string[]).includes(theme)) {
      root.classList.add(`theme-${theme}`, "theme-light");
      return;
    }

    root.classList.add(theme);
  }, [theme]);

  const value = {
    theme,
    setTheme: (theme: Theme) => {
      localStorage.setItem(storageKey, theme);
      setTheme(theme);
    },
  };

  return (
    <ThemeProviderContext.Provider {...props} value={value}>
      {children}
    </ThemeProviderContext.Provider>
  );
}

export const useTheme = () => {
  const context = useContext(ThemeProviderContext);

  if (context === undefined)
    throw new Error("useTheme must be used within a ThemeProvider");

  return context;
};
