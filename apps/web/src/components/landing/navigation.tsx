"use client";

import { Menu, X } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";

const navLinks = [
  { name: "Features", href: "#features" },
  { name: "How It Works", href: "#how-it-works" },
  { name: "Database", href: "#infra" },
  { name: "Integrations", href: "#integrations" },
];

export function Navigation() {
  const [isScrolled, setIsScrolled] = useState(false);
  const [isMobileMenuOpen, setIsMobileMenuOpen] = useState(false);

  useEffect(() => {
    const handleScroll = () => {
      setIsScrolled(window.scrollY > 20);
    };
    window.addEventListener("scroll", handleScroll);
    return () => window.removeEventListener("scroll", handleScroll);
  }, []);

  return (
    <header
      className={`fixed z-50 transition-all duration-500 ${
        isScrolled ? "top-4 left-4 right-4" : "top-0 left-0 right-0"
      }`}
    >
      <nav
        className={`mx-auto transition-all duration-500 ${
          isScrolled || isMobileMenuOpen
            ? "bg-background/80 backdrop-blur-xl border border-foreground/10 rounded-2xl shadow-lg max-w-300"
            : "bg-transparent max-w-350"
        }`}
      >
        <div
          className={`flex items-center justify-between transition-all duration-500 px-6 lg:px-8 ${
            isScrolled ? "h-14" : "h-20"
          }`}
        >
          {/* Logo */}
          <a href="/#" className="flex items-center gap-2 group">
            <span
              className={`font-display tracking-tight transition-all duration-500 ${isScrolled ? "text-xl text-foreground" : "text-2xl text-white"}`}
            >
              VELOPULENT
            </span>
            <span
              className={`font-mono transition-all duration-500 ${isScrolled ? "text-[10px] mt-0.5 text-muted-foreground" : "text-xs mt-1 text-white/60"}`}
            >
              CMS
            </span>
          </a>

          {/* Desktop Navigation */}
          <div className="hidden xl:flex items-center gap-12">
            {navLinks.map((link) => (
              <a
                key={link.name}
                href={link.href}
                className={`text-sm transition-colors duration-300 relative group ${isScrolled ? "text-foreground/70 hover:text-foreground" : "text-white/70 hover:text-white"}`}
              >
                {link.name}
                <span
                  className={`absolute -bottom-1 left-0 w-0 h-px transition-all duration-300 group-hover:w-full ${isScrolled ? "bg-foreground" : "bg-white"}`}
                />
              </a>
            ))}
          </div>

          {/* Desktop CTA */}
          <div className="hidden xl:flex items-center gap-4">
            <a
              href="/docs"
              className={`transition-all duration-500 ${isScrolled ? "text-xs text-foreground/70 hover:text-foreground" : "text-sm text-white/70 hover:text-white"}`}
            >
              Documentation
            </a>

            {/* TODO: replace with Download button */}
            <Button
              size="sm"
              className={`rounded-full transition-all duration-500 ${isScrolled ? "bg-foreground hover:bg-foreground/90 text-background px-4 h-8 text-xs" : "bg-white hover:bg-white/90 text-black px-6"}`}
            >
              <Link href={"https://github.com/velopulent/cms"}>Github</Link>
            </Button>
          </div>

          {/* Mobile Menu Button */}
          <button
            type="button"
            onClick={() => setIsMobileMenuOpen(!isMobileMenuOpen)}
            className={`z-50 xl:hidden p-2 transition-colors duration-500 ${isScrolled || isMobileMenuOpen ? "text-foreground" : "text-white"}`}
            aria-label="Toggle menu"
          >
            {isMobileMenuOpen ? (
              <X className="w-6 h-6" />
            ) : (
              <Menu className="w-6 h-6" />
            )}
          </button>
        </div>
      </nav>

      {/* Mobile Menu - Full Screen Overlay */}
      <div
        className={`xl:hidden -z-1 fixed inset-0 bg-background transition-all duration-500 ${
          isMobileMenuOpen
            ? "opacity-100 pointer-events-auto"
            : "opacity-0 pointer-events-none"
        }`}
        style={{ top: 0 }}
      >
        <div className="flex flex-col h-full px-8 pt-28 pb-8">
          {/* Navigation Links */}
          <div className="flex-1 flex flex-col justify-center gap-8">
            {navLinks.map((link, i) => (
              <a
                key={link.name}
                href={link.href}
                onClick={() => setIsMobileMenuOpen(false)}
                className={`text-5xl font-display text-foreground hover:text-muted-foreground transition-all duration-500 ${
                  isMobileMenuOpen
                    ? "opacity-100 translate-y-0"
                    : "opacity-0 translate-y-4"
                }`}
                style={{
                  transitionDelay: isMobileMenuOpen ? `${i * 75}ms` : "0ms",
                }}
              >
                {link.name}
              </a>
            ))}
          </div>

          {/* Bottom CTAs */}
          <div
            className={`flex gap-4 pt-8 border-t border-foreground/10 transition-all duration-500 ${
              isMobileMenuOpen
                ? "opacity-100 translate-y-0"
                : "opacity-0 translate-y-4"
            }`}
            style={{ transitionDelay: isMobileMenuOpen ? "300ms" : "0ms" }}
          >
            <Button
              variant="outline"
              className="flex-1 rounded-full h-14 text-base"
              onClick={() => setIsMobileMenuOpen(false)}
              asChild
            >
              <a href={"/docs"}>Documentation</a>
            </Button>
            {/* TODO: replace with Download button */}
            <Button
              className="flex-1 bg-foreground text-background rounded-full h-14 text-base"
              onClick={() => setIsMobileMenuOpen(false)}
            >
              <Link href={"https://github.com/velopulent/cms"}>Github</Link>
            </Button>
          </div>
        </div>
      </div>
    </header>
  );
}
