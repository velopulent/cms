import { File, Files, Folder } from "fumadocs-ui/components/files";
import { Steps } from "fumadocs-ui/components/steps";
import defaultMdxComponents from "fumadocs-ui/mdx";
import {
  Blocks,
  BookOpenCheck,
  Braces,
  Cable,
  KeyRound,
  Newspaper,
  PackageOpen,
  Rocket,
  ServerCog,
  ShieldCheck,
  SlidersHorizontal,
} from "lucide-react";
import type { MDXComponents } from "mdx/types";
import { AdaptiveScreenshot } from "@/components/adaptive-screenshot";

export function getMDXComponents(components?: MDXComponents) {
  return {
    ...defaultMdxComponents,
    AdaptiveScreenshot,
    Blocks,
    BookOpenCheck,
    Braces,
    Cable,
    File,
    Files,
    Folder,
    KeyRound,
    Newspaper,
    PackageOpen,
    Rocket,
    ServerCog,
    ShieldCheck,
    SlidersHorizontal,
    Steps,
    ...components,
  } satisfies MDXComponents;
}

export const useMDXComponents = getMDXComponents;

declare global {
  type MDXProvidedComponents = ReturnType<typeof getMDXComponents>;
}
