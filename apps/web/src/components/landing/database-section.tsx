"use client";

import Image from "next/image";
import { useEffect, useRef, useState } from "react";

const regions = [
  { name: "SQLite", nodes: 0, status: "supported" },
  { name: "PostgreSQL", nodes: 0, status: "supported" },
];

export function DatabaseSection() {
  const [isVisible, setIsVisible] = useState(false);
  const [activeRegion, setActiveRegion] = useState(0);
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

  useEffect(() => {
    const interval = setInterval(() => {
      setActiveRegion((prev) => (prev + 1) % regions.length);
    }, 3000);
    return () => clearInterval(interval);
  }, []);

  const lineConfigs = Array.from({ length: 19 }, (_, i) => ({
    x1: 10 + (i % 5) * 20,
    y1: 10 + Math.floor(i / 5) * 25,
    x2: 10 + ((i + 1) % 5) * 20,
    y2: 10 + Math.floor((i + 1) / 5) * 25,
    delay: i * 0.15,
  }));

  const dotConfigs = Array.from({ length: 20 }, (_, i) => ({
    left: 10 + (i % 5) * 20,
    top: 10 + Math.floor(i / 5) * 25,
    delay: i * 0.1,
  }));

  return (
    <section
      id="infra"
      ref={sectionRef}
      className="relative py-32 lg:py-40 overflow-hidden"
    >
      {/* Background accent — retiré, remplacé par l'image sphère */}

      <div className="max-w-350 mx-auto px-6 lg:px-12">
        {/* Header */}
        <div className="mb-20">
          <span
            className={`inline-flex items-center gap-4 text-sm font-mono text-muted-foreground mb-8 transition-all duration-700 ${
              isVisible ? "opacity-100" : "opacity-0"
            }`}
          >
            <span className="w-12 h-px bg-foreground/20" />
            Database Support
          </span>

          <div className="grid lg:grid-cols-[auto_1fr] gap-8 lg:gap-16 items-stretch">
            {/* Image globe — colonne gauche, pleine hauteur */}
            <div
              className={`w-48 lg:w-72 xl:w-80 shrink-0 relative transition-all duration-1000 ${
                isVisible
                  ? "opacity-100 translate-y-0"
                  : "opacity-0 translate-y-8"
              }`}
            >
              <Image
                fill
                src="/assets/database.png"
                alt="Global network sphere"
                className="object-contain object-center"
              />
            </div>

            {/* Titre + description empilés */}
            <div className="flex flex-col justify-center">
              <h2
                className={`text-6xl md:text-7xl lg:text-[128px] font-display tracking-tight leading-[0.9] transition-all duration-1000 ${
                  isVisible
                    ? "opacity-100 translate-y-0"
                    : "opacity-0 translate-y-8"
                }`}
              >
                Your choice
                <br />
                <span className="text-muted-foreground">of database.</span>
              </h2>

              <p
                className={`mt-8 text-xl text-muted-foreground leading-relaxed max-w-lg transition-all duration-1000 delay-100 ${
                  isVisible ? "opacity-100" : "opacity-0"
                }`}
              >
                Embedded SQLite for lightweight workloads and PostgreSQL for
                scalable, high-volume systems.
              </p>
            </div>
          </div>
        </div>

        {/* Main content grid */}
        <div className="grid lg:grid-cols-3 gap-6">
          {/* Large stat card */}
          <div
            className={`lg:col-span-2 relative p-8 lg:p-12 border border-foreground/10 bg-foreground/2 overflow-hidden transition-all duration-700 ${
              isVisible
                ? "opacity-100 translate-y-0"
                : "opacity-0 translate-y-8"
            }`}
          >
            {/* Animated dots background with connecting lines */}
            <div className="absolute inset-0 opacity-70">
              {/* SVG for connecting lines */}
              <svg
                aria-hidden="true"
                className="absolute inset-0 w-full h-full"
                style={{ pointerEvents: "none" }}
              >
                <defs>
                  <style>{`
                    @keyframes drawLine {
                      0%   { stroke-dashoffset: 1000; opacity: 0; }
                      15%  { opacity: 1; }
                      70%  { opacity: 0.7; }
                      100% { stroke-dashoffset: 0; opacity: 0; }
                    }
                    .connecting-line {
                      stroke: #eca8d6;
                      stroke-width: 1.2;
                      fill: none;
                      stroke-dasharray: 1000;
                      animation: drawLine 3s ease-in-out infinite;
                    }
                  `}</style>
                </defs>
                {lineConfigs.map((cfg) => (
                  <line
                    key={`${cfg.x1}-${cfg.y1}`}
                    x1={`${cfg.x1}%`}
                    y1={`${cfg.y1}%`}
                    x2={`${cfg.x2}%`}
                    y2={`${cfg.y2}%`}
                    className="connecting-line"
                    style={{ animationDelay: `${cfg.delay}s` }}
                  />
                ))}
              </svg>

              {/* Dots */}
              {dotConfigs.map((dot) => (
                <div
                  key={`${dot.left}-${dot.top}`}
                  className="absolute w-1.5 h-1.5 rounded-full bg-[#eca8d6]"
                  style={{
                    left: `${dot.left}%`,
                    top: `${dot.top}%`,
                    animation: `pulse 2s ease-in-out ${dot.delay}s infinite`,
                  }}
                />
              ))}
            </div>

            <div className="relative z-10">
              <div className="flex items-baseline gap-2 mb-4">
                <span className="text-8xl lg:text-[10rem] font-display leading-none">
                  2
                </span>
                <span className="text-2xl text-muted-foreground">
                  databases
                </span>
              </div>
              <p className="text-muted-foreground max-w-md">
                Built-in support for the most popular databases. Configure once,
                scale anywhere.
              </p>
            </div>
          </div>

          {/* Stacked stat cards */}
          <div className="flex flex-col gap-6">
            <div
              className={`p-8 border border-foreground/10 bg-foreground/2 transition-all duration-700 delay-100 ${
                isVisible
                  ? "opacity-100 translate-y-0"
                  : "opacity-0 translate-y-8"
              }`}
            >
              <span className="text-5xl lg:text-6xl font-display">
                &lt; 20MB
              </span>
              <span className="block text-sm text-muted-foreground mt-2">
                Binary size
              </span>
            </div>

            <div
              className={`p-8 border border-foreground/10 bg-foreground/2 transition-all duration-700 delay-200 ${
                isVisible
                  ? "opacity-100 translate-y-0"
                  : "opacity-0 translate-y-8"
              }`}
            >
              <span className="text-5xl lg:text-6xl font-display">∞</span>
              <span className="block text-sm text-muted-foreground mt-2">
                Projects per instance
              </span>
            </div>
          </div>
        </div>

        {/* Region list */}
        <div
          className={`mt-12 grid grid-cols-2 gap-4 transition-all duration-1000 delay-300 ${
            isVisible ? "opacity-100" : "opacity-0"
          }`}
        >
          {regions.map((region, index) => (
            <div
              key={region.name}
              className={`p-6 border transition-all duration-300 cursor-default ${
                activeRegion === index
                  ? "border-foreground/30 bg-foreground/4"
                  : "border-foreground/10"
              }`}
            >
              <div className="flex items-center gap-2 mb-3">
                <span
                  className={`w-2 h-2 rounded-full transition-colors ${
                    activeRegion === index ? "bg-[#eca8d6]" : "bg-foreground/20"
                  }`}
                />
                <span className="text-xs font-mono text-muted-foreground uppercase tracking-wider">
                  {region.status}
                </span>
              </div>
              <span className="font-medium block mb-1">{region.name}</span>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
