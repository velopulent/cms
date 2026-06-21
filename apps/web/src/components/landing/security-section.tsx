"use client";

import { Eye, FileCheck, Lock, Shield } from "lucide-react";
import { useEffect, useRef, useState } from "react";

const securityFeatures = [
  {
    icon: Shield,
    title: "Role-based access",
    description: "Admin, editor, and viewer roles for teams.",
    image: "/assets/isolated.jpg",
  },
  {
    icon: Lock,
    title: "Secure by default",
    description: "Content encrypted at rest. HTTPS for all APIs.",
    image: "/assets/encrypted.jpg",
  },
  {
    icon: Eye,
    title: "Full version control",
    description: "Track all changes. Restore any previous version.",
    image: "/assets/audit.jpg",
  },
  {
    icon: FileCheck,
    title: "Open source auditable",
    description: "Review the code. No black boxes. Full transparency.",
    image: "/assets/permissions.jpg",
  },
];

const certifications = ["Open Source", "AGPL v3 License"];

export function SecuritySection() {
  const [isVisible, setIsVisible] = useState(false);
  const [activeFeature, setActiveFeature] = useState(0);
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
      setActiveFeature((prev) => (prev + 1) % securityFeatures.length);
    }, 3000);
    return () => clearInterval(interval);
  }, []);

  return (
    <section
      id="security"
      ref={sectionRef}
      className="relative py-32 lg:py-40 overflow-hidden"
    >
      {/* Background accent removed */}

      <div className="max-w-350 mx-auto px-6 lg:px-12">
        {/* Header */}
        <div className="mb-20">
          <span
            className={`inline-flex items-center gap-4 text-sm font-mono text-muted-foreground mb-8 transition-all duration-700 ${
              isVisible ? "opacity-100" : "opacity-0"
            }`}
          >
            <span className="w-12 h-px bg-foreground/20" />
            Security
          </span>

          {/* Title — full width */}
          <h2
            className={`text-6xl md:text-7xl lg:text-[128px] font-display tracking-tight leading-[0.9] mb-12 transition-all duration-1000 ${
              isVisible
                ? "opacity-100 translate-y-0"
                : "opacity-0 translate-y-8"
            }`}
          >
            Built for trust.
            <br />
            <span className="text-muted-foreground">Auditable and open.</span>
          </h2>

          {/* Description — below title */}
          <div
            className={`transition-all duration-1000 delay-100 ${
              isVisible ? "opacity-100" : "opacity-0"
            }`}
          >
            <p className="text-xl text-muted-foreground leading-relaxed max-w-2xl">
              100% open source means you control your data. Role-based access
              control for teams. Complete audit history of all changes.
            </p>
          </div>
        </div>

        {/* Main content */}
        <div className="grid lg:grid-cols-12 gap-6">
          {/* Large visual card */}
          <div
            className={`lg:col-span-7 relative p-8 lg:p-12 border border-foreground/10 min-h-100 overflow-hidden transition-all duration-700 ${
              isVisible
                ? "opacity-100 translate-y-0"
                : "opacity-0 translate-y-8"
            }`}
          >
            {/* Dynamic feature image with cross-fade — desktop only */}
            <div className="absolute inset-0 pointer-events-none items-center justify-end hidden lg:flex">
              {securityFeatures.map((feature, index) => (
                <img
                  key={feature.image}
                  src={feature.image}
                  alt={feature.title}
                  className="absolute h-3/4 w-3/4 object-contain object-right transition-opacity duration-500"
                  style={{ opacity: activeFeature === index ? 0.85 : 0 }}
                />
              ))}
            </div>

            <div className="relative z-10">
              <span className="font-mono text-sm text-muted-foreground">
                Your data
              </span>
              <div className="mt-8">
                <span className="text-7xl lg:text-8xl font-display">100%</span>
                <span className="block text-muted-foreground mt-2">
                  Under your control
                </span>
              </div>
            </div>

            {/* Certification badges */}
            <div className="absolute bottom-8 left-8 right-8 flex flex-wrap gap-2">
              {certifications.map((cert, index) => (
                <span
                  key={cert}
                  className={`px-3 py-1 border border-foreground/10 text-xs font-mono text-muted-foreground transition-all duration-500 ${
                    isVisible
                      ? "opacity-100 translate-y-0"
                      : "opacity-0 translate-y-4"
                  }`}
                  style={{ transitionDelay: `${index * 100 + 300}ms` }}
                >
                  {cert}
                </span>
              ))}
            </div>
          </div>

          {/* Feature cards stack */}
          <div className="lg:col-span-5 flex flex-col gap-4">
            {securityFeatures.map((feature, index) => (
              <button
                type="button"
                key={feature.title}
                className={`p-6 border transition-all duration-500 cursor-default text-left w-full ${
                  activeFeature === index
                    ? "border-foreground/30 bg-foreground/4"
                    : "border-foreground/10"
                } ${isVisible ? "opacity-100 translate-x-0" : "opacity-0 translate-x-8"}`}
                style={{ transitionDelay: `${index * 80}ms` }}
                onClick={() => setActiveFeature(index)}
                onMouseEnter={() => setActiveFeature(index)}
              >
                <div className="flex items-start gap-4">
                  <div
                    className={`shrink-0 w-10 h-10 flex items-center justify-center border transition-colors ${
                      activeFeature === index
                        ? "border-foreground bg-foreground text-background"
                        : "border-foreground/20"
                    }`}
                  >
                    <feature.icon className="w-5 h-5" />
                  </div>
                  <div>
                    <h3 className="font-medium mb-1">{feature.title}</h3>
                    <p className="text-sm text-muted-foreground">
                      {feature.description}
                    </p>
                  </div>
                </div>
              </button>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}
