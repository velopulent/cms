"use client";

import { ArrowUpRight } from "lucide-react";
import Link from "next/link";

const footerLinks = {
  Product: [
    { name: "Features", href: "#features" },
    { name: "Integrations", href: "#integrations" },
    { name: "Documentation", href: "/docs" },
    { name: "GitHub", href: "https://github.com/velopulent/cms" },
  ],
  Resources: [
    { name: "Installation", href: "/docs" },
    { name: "API Reference", href: "/docs/api-reference" },
    {
      name: "Examples",
      href: "/docs/examples",
    },
    { name: "Community", href: "/community" },
  ],
  Company: [
    { name: "About", href: "https://velopulent.com/about" },
    { name: "Blog", href: "https://velopulent.com/blog" },
    { name: "Products", href: "https://velopulent.com/products" },
    { name: "Contact", href: "https://velopulent.com/contact" },
  ],
  Legal: [
    { name: "Privacy", href: "/privacy" },
    { name: "Terms", href: "/terms" },
    {
      name: "License",
      href: "https://github.com/velopulent/cms/blob/main/LICENSE",
    },
  ],
};

const socialLinks = [
  { name: "X / Twitter", href: "https://x.com/VelopulentHQ" },
  { name: "GitHub", href: "https://github.com/velopulent" },
  { name: "Youtube", href: "https://youtube.com/@velopulent" },
];

export function FooterSection() {
  return (
    <footer className="relative bg-black">
      {/* Panoramic banner image */}
      <div className="relative w-full h-85 md:h-105 overflow-hidden">
        <img
          src="/assets/footer.png"
          alt="Bioluminescent landscape"
          className="w-full h-full object-cover object-center"
        />
        {/* Gradient fade to black at bottom */}
        <div className="absolute inset-0 bg-linear-to-b from-transparent via-transparent to-black" />
        {/* Subtle dark vignette on sides */}
        <div className="absolute inset-0 bg-linear-to-r from-black/40 via-transparent to-black/40" />
      </div>

      {/* Footer content — black background, white text */}
      <div className="relative z-10 max-w-350 mx-auto px-6 lg:px-12">
        {/* Main Footer */}
        <div className="py-16 lg:py-20">
          <div className="grid grid-cols-2 md:grid-cols-6 gap-12 lg:gap-8">
            {/* Brand Column */}
            <div className="col-span-2">
              <Link href="/" className="inline-flex items-center gap-2 mb-6">
                <span className="text-2xl font-display text-white">
                  VELOPULENT
                </span>
                <span className="text-xs text-white/40 font-mono">CMS</span>
              </Link>

              <p className="text-white/50 leading-relaxed mb-8 max-w-xs text-sm">
                Open-source headless CMS. Self-hosted. Built with Rust. Fast and
                reliable.
              </p>

              {/* Social Links */}
              <div className="flex gap-6">
                {socialLinks.map((link) => (
                  <a
                    key={link.name}
                    href={link.href}
                    target="_blank"
                    className="text-sm text-white/40 hover:text-white transition-colors flex items-center gap-1 group"
                    rel="noopener"
                  >
                    {link.name}
                    <ArrowUpRight className="w-3 h-3 opacity-0 -translate-x-1 group-hover:opacity-100 group-hover:translate-x-0 transition-all" />
                  </a>
                ))}
              </div>
            </div>

            {/* Link Columns */}
            {Object.entries(footerLinks).map(([title, links]) => (
              <div key={title}>
                <h3 className="text-sm font-medium text-white mb-6">{title}</h3>
                <ul className="space-y-4">
                  {links.map((link) => (
                    <li key={link.name}>
                      <a
                        href={link.href}
                        className="text-sm text-white/40 hover:text-white transition-colors inline-flex items-center gap-2"
                      >
                        {link.name}
                      </a>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        </div>

        {/* Bottom Bar */}
        <div className="py-8 border-t border-white/10 flex flex-col md:flex-row items-center justify-between gap-4">
          <p className="text-sm text-white/30">
            &copy; {new Date().getFullYear()} Velopulent LLP
          </p>

          <p className="text-sm text-white/30">
            Open source under AGPL v3 license.
          </p>

          <div className="flex items-center gap-4 text-sm text-white/30">
            <span className="flex items-center gap-2">
              A Product by Velopulent
            </span>
          </div>
        </div>
      </div>
    </footer>
  );
}
