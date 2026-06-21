import "./global.css";
import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Velopulent CMS",
  description: "Headless Content Management System by Velopulent",
  metadataBase: "https://cms.velopulent.com",
};

export default function Layout({ children }: LayoutProps<"/">) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body>{children}</body>
    </html>
  );
}
