import "./docs.css";
import { source } from "@/lib/source";
import { DocsLayout } from "fumadocs-ui/layouts/docs";
import { baseOptions } from "@/lib/layout.shared";
import { Provider as FumadocsProvider } from "@/components/provider";
import { Geist, Inter } from "next/font/google";
import { cn } from "@/lib/cn";

const geist = Geist({ subsets: ["latin"], variable: "--font-sans" });

const inter = Inter({
  subsets: ["latin"],
});

export default function Layout({ children }: LayoutProps<"/docs">) {
  return (
    <div
      className={`${cn(inter.className, "font-sans", geist.variable)} flex flex-col min-h-screen`}
    >
      <FumadocsProvider>
        <DocsLayout tree={source.getPageTree()} {...baseOptions()}>
          {children}
        </DocsLayout>
      </FumadocsProvider>
    </div>
  );
}
