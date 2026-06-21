"use client";

import { Icon } from "@iconify-icon/react";
import { LucideArrowRight } from "lucide-react";
import Image from "next/image";
import { useEffect, useRef, useState } from "react";

const frameworks = [
  { name: "Next.js", icon: "logos:nextjs-icon" },
  { name: "React", icon: "logos:react" },
  { name: "Vue", icon: "logos:vue" },
  { name: "Nuxt", icon: "logos:nuxt-icon" },
  { name: "Svelte", icon: "logos:svelte-icon" },
  { name: "Astro", icon: "logos:astro-icon" },
  { name: "Angular", icon: "logos:angular-icon" },
  { name: "React Router", icon: "logos:react-router" },
  { name: "Gatsby", icon: "logos:gatsby" },
  { name: "SolidJS", icon: "logos:solidjs-icon" },
  { name: "Qwik", icon: "logos:qwik-icon" },
  { name: "Ember", icon: "logos:ember-tomster" },
];

export function IntegrationsSection() {
  const [isVisible, setIsVisible] = useState(false);
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null);
  const [mousePos, setMousePos] = useState<{ x: number; y: number } | null>(
    null,
  );
  const sectionRef = useRef<HTMLElement>(null);

  useEffect(() => {
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) setIsVisible(true);
      },
      { threshold: 0.1 },
    );

    if (sectionRef.current) observer.observe(sectionRef.current);
    return () => observer.disconnect();
  }, []);

  return (
    <section
      id="integrations"
      ref={sectionRef}
      className="relative overflow-hidden"
    >
      {/* Header — centré verticalement sur l'image */}
      <div className="relative z-10 pt-32 lg:pt-40 text-center">
        <span
          className={`inline-flex items-center gap-4 text-sm font-mono text-muted-foreground mb-8 transition-all duration-700 justify-center ${
            isVisible ? "opacity-100" : "opacity-0"
          }`}
        >
          <span className="w-12 h-px bg-foreground/20" />
          Integrations
          <span className="w-12 h-px bg-foreground/20" />
        </span>

        <h2
          className={`text-6xl md:text-7xl lg:text-[128px] font-display tracking-tight leading-[0.9] transition-all duration-1000 ${
            isVisible ? "opacity-100 translate-y-0" : "opacity-0 translate-y-8"
          }`}
        >
          Works with
          <br />
          <span className="text-muted-foreground">everything.</span>
        </h2>

        <p
          className={`mt-8 text-xl text-muted-foreground leading-relaxed max-w-lg mx-auto transition-all duration-1000 delay-100 ${
            isVisible ? "opacity-100" : "opacity-0"
          }`}
        >
          Connect to any framework, any static site generator, and any
          deployment platform. Multiple APIs available for flexibility.
        </p>
      </div>

      {/* Full-width image */}
      <div
        className={`relative left-1/2 -translate-x-1/2 w-screen -mt-16 transition-all duration-1000 delay-200 ${
          isVisible ? "opacity-100" : "opacity-0"
        }`}
        style={{ aspectRatio: "2494 / 1199" }}
      >
        <Image
          fill
          src="/assets/connection.png"
          alt=""
          aria-hidden="true"
          className="object-cover"
        />
      </div>

      {/* Integration grid — remonte sur l'image avec spacing mobile approprié */}
      <div className="relative z-10 mt-0 lg:-mt-24 max-w-350 mx-auto px-6 lg:px-12">
        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4 mb-16">
          {frameworks.map((framework, index) => (
            <div
              key={framework.name}
              role="none"
              className={`group relative overflow-hidden p-6 lg:p-8 border transition-all duration-500 cursor-default ${
                hoveredIndex === index
                  ? "border-foreground bg-foreground/4 scale-[1.02]"
                  : "border-foreground/10 hover:border-foreground/30"
              } ${isVisible ? "opacity-100 translate-y-0" : "opacity-0 translate-y-8"}`}
              style={{
                transitionDelay: `${index * 30 + 300}ms`,
              }}
              onMouseEnter={(e) => {
                setHoveredIndex(index);
                const rect = e.currentTarget.getBoundingClientRect();
                setMousePos({
                  x: e.clientX - rect.left,
                  y: e.clientY - rect.top,
                });
              }}
              onMouseMove={(e) => {
                const rect = e.currentTarget.getBoundingClientRect();
                setMousePos({
                  x: e.clientX - rect.left,
                  y: e.clientY - rect.top,
                });
              }}
              onMouseLeave={() => {
                setHoveredIndex(null);
                setMousePos(null);
              }}
            >
              {/* Cursor-following halo */}
              {hoveredIndex === index && mousePos && (
                <span
                  aria-hidden="true"
                  className="pointer-events-none absolute inset-0 z-0"
                  style={{
                    background: `radial-gradient(200px circle at ${mousePos.x}px ${mousePos.y}px, rgba(255,255,255,0.1) 0%, transparent 70%)`,
                  }}
                />
              )}

              {/* Logo */}
              <div
                className={`w-10 h-10 mb-6 flex items-center justify-center transition-colors ${
                  hoveredIndex === index ? "text-white" : "text-foreground/60"
                }`}
              >
                <Icon icon={framework.icon} width="40" height="40" />
              </div>

              <span className="font-medium block">{framework.name}</span>

              {/* Animated underline */}
              <div className="absolute bottom-0 left-0 right-0 h-px bg-foreground/20 overflow-hidden">
                <div
                  className={`h-full bg-foreground transition-all duration-500 ${
                    hoveredIndex === index ? "w-full" : "w-0"
                  }`}
                />
              </div>
            </div>
          ))}
        </div>

        {/* Bottom stats row */}
        <div
          className={`flex flex-wrap items-center justify-between gap-8 pt-12 border-t border-foreground/10 transition-all duration-1000 delay-500 pb-32 lg:pb-40 ${
            isVisible ? "opacity-100" : "opacity-0"
          }`}
        >
          <div className="flex flex-wrap gap-12">
            {[
              { value: "REST", label: "GraphQL & more" },
              { value: "Webhooks", label: "For build triggers" },
              { value: "Open", label: "API & Extensible" },
            ].map((stat) => (
              <div key={stat.label} className="flex items-baseline gap-3">
                <span className="text-3xl font-display">{stat.value}</span>
                <span className="text-sm text-muted-foreground">
                  {stat.label}
                </span>
              </div>
            ))}
          </div>

          <a
            href="https://github.com/velopulent/cms"
            className="group inline-flex items-center gap-1 text-sm font-mono text-muted-foreground hover:text-foreground transition-colors"
          >
            View on GitHub
            <span className="group-hover:translate-x-1 transition-transform">
              <LucideArrowRight size={12} />
            </span>
          </a>
        </div>
      </div>
    </section>
  );
}
