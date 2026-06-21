"use client";

import { Cloud, HardDrive, RotateCcw, Shield } from "lucide-react";
import Image from "next/image";
import { useEffect, useRef, useState } from "react";

const backupFeatures = [
  {
    icon: Cloud,
    title: "S3 Compatible",
    description:
      "Backup to AWS S3, MinIO, DigitalOcean Spaces, or any S3-compatible storage.",
  },
  {
    icon: HardDrive,
    title: "Local Storage",
    description:
      "Store backups directly on your filesystem for complete control.",
  },
  {
    icon: Shield,
    title: "Encrypted Backups",
    description:
      "All backups are encrypted and can be stored securely anywhere.",
  },
  {
    icon: RotateCcw,
    title: "One-Click Restore",
    description: "Restore your entire database and files with a single click.",
  },
];

export function BackupSection() {
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
      id="backup"
      ref={sectionRef}
      className="relative py-32 lg:py-40 overflow-hidden"
    >
      <div className="max-w-350 mx-auto px-6 lg:px-12">
        {/* Header */}
        <div className="mb-20">
          <span
            className={`inline-flex items-center gap-4 text-sm font-mono text-muted-foreground mb-8 transition-all duration-700 ${
              isVisible ? "opacity-100" : "opacity-0"
            }`}
          >
            <span className="w-12 h-px bg-foreground/20" />
            Data Protection
          </span>

          <div className="grid lg:grid-cols-[auto_1fr] gap-8 lg:gap-16 items-stretch">
            {/* Image — left column */}
            <div
              className={`w-48 lg:w-72 xl:w-80 shrink-0 relative transition-all duration-1000 ${
                isVisible
                  ? "opacity-100 translate-y-0"
                  : "opacity-0 translate-y-8"
              }`}
            >
              <Image
                fill
                src="/assets/backup.png"
                alt="Backup and restore"
                className="object-contain"
              />
            </div>

            {/* Title + description */}
            <div className="flex flex-col justify-center">
              <h2
                className={`text-6xl md:text-7xl lg:text-[128px] font-display tracking-tight leading-[0.9] transition-all duration-1000 ${
                  isVisible
                    ? "opacity-100 translate-y-0"
                    : "opacity-0 translate-y-8"
                }`}
              >
                Backup &
                <br />
                <span className="text-muted-foreground">restore freely.</span>
              </h2>

              <p
                className={`mt-8 text-xl text-muted-foreground leading-relaxed max-w-lg transition-all duration-1000 delay-100 ${
                  isVisible ? "opacity-100" : "opacity-0"
                }`}
              >
                Never lose your data. Automated backups to your choice of
                storage, encrypted for security. Restore anytime with zero
                downtime.
              </p>
            </div>
          </div>
        </div>

        {/* Features grid */}
        <div className="grid md:grid-cols-2 lg:grid-cols-4 gap-6">
          {backupFeatures.map((feature, index) => {
            const Icon = feature.icon;
            return (
              <div
                key={feature.title}
                role="none"
                className={`group relative overflow-hidden p-6 lg:p-8 border transition-all duration-500 cursor-default ${
                  hoveredIndex === index
                    ? "border-foreground bg-foreground/4"
                    : "border-foreground/10 hover:border-foreground/30"
                } ${isVisible ? "opacity-100 translate-y-0" : "opacity-0 translate-y-8"}`}
                style={{
                  transitionDelay: `${index * 50 + 300}ms`,
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

                {/* Icon */}
                <div
                  className={`w-10 h-10 mb-6 flex items-center justify-center transition-colors ${
                    hoveredIndex === index ? "text-white" : "text-foreground/60"
                  }`}
                >
                  <Icon className="w-6 h-6" />
                </div>

                <h3 className="font-medium mb-3">{feature.title}</h3>
                <p className="text-sm text-muted-foreground leading-relaxed">
                  {feature.description}
                </p>

                {/* Animated underline */}
                <div className="absolute bottom-0 left-0 right-0 h-px bg-foreground/20 overflow-hidden">
                  <div
                    className={`h-full bg-foreground transition-all duration-500 ${
                      hoveredIndex === index ? "w-full" : "w-0"
                    }`}
                  />
                </div>
              </div>
            );
          })}
        </div>

        {/* Bottom highlight box */}
        <div
          className={`mt-16 p-8 lg:p-12 border border-foreground/10 bg-foreground/[0.02] transition-all duration-1000 delay-500 ${
            isVisible ? "opacity-100 translate-y-0" : "opacity-0 translate-y-8"
          }`}
        >
          <div className="grid md:grid-cols-3 gap-8">
            <div>
              <span className="text-4xl lg:text-5xl font-display">
                Automated
              </span>
              <p className="text-muted-foreground mt-2">
                Set scheduled backups and forget about it
              </p>
            </div>
            <div>
              <span className="text-4xl lg:text-5xl font-display">
                Flexible
              </span>
              <p className="text-muted-foreground mt-2">
                Choose any storage provider you trust
              </p>
            </div>
            <div>
              <span className="text-4xl lg:text-5xl font-display">Safe</span>
              <p className="text-muted-foreground mt-2">
                Encrypted, versioned, and independently verified
              </p>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
