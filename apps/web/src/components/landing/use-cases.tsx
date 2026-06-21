"use client";

import { useEffect, useRef, useState } from "react";

const useCases = [
  {
    title: "Blog & Articles",
    description:
      "Full-featured blogging with rich text, categories, and scheduling.",
  },
  {
    title: "Documentation",
    description:
      "Organize docs with hierarchical structure and version management.",
  },
  {
    title: "E-commerce",
    description:
      "Product catalogs with variants, pricing, and inventory tracking.",
  },
  {
    title: "Landing Pages",
    description: "Build beautiful landing pages with flexible content blocks.",
  },
  {
    title: "News & Updates",
    description: "Publish breaking news and updates with real-time publishing.",
  },
  {
    title: "Portfolio",
    description: "Showcase projects, case studies, and work samples elegantly.",
  },
];

export function UseCasesSection() {
  const [isVisible, setIsVisible] = useState(false);
  const sectionRef = useRef<HTMLElement | null>(null);

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
      id="use-cases"
      ref={sectionRef}
      className="relative py-24 lg:py-32 overflow-hidden"
    >
      {/* IMAGE LAYER */}
      <div
        className={`
          pointer-events-none absolute inset-0 transition-all duration-1000
          ${isVisible ? "opacity-100" : "opacity-0"}
          
          /* MOBILE: full background */
          w-full h-full
          
          /* DESKTOP: right-side layout */
          lg:inset-auto lg:bottom-0 lg:right-0 
          lg:w-[55%] lg:h-[85%]
        `}
      >
        <img
          src="/assets/tree-house-2x.png"
          aria-hidden="true"
          className={`
            w-full h-full object-cover
            
            /* MOBILE: centered bg */
            object-center
            
            /* DESKTOP: original alignment */
            lg:object-top-left
          `}
        />

        {/* MOBILE overlay (strong for readability) */}
        <div className="absolute inset-0 bg-linear-to-b from-black/75 via-black/90 to-black/95 lg:hidden" />

        {/* DESKTOP fades (your original) */}
        <div className="hidden lg:block absolute inset-0 bg-linear-to-r from-background via-background/60 to-transparent" />
        <div className="hidden lg:block absolute inset-0 bg-linear-to-b from-background via-transparent to-transparent" />
      </div>

      {/* CONTENT */}
      <div className="relative z-10 max-w-350 mx-auto px-6 lg:px-12">
        {/* Header */}
        <div
          className={`mb-16 transition-all duration-700 ${
            isVisible ? "opacity-100 translate-y-0" : "opacity-0 translate-y-8"
          }`}
        >
          <span className="inline-flex items-center gap-3 text-sm font-mono text-muted-foreground mb-6">
            <span className="w-8 h-px bg-foreground/30" />
            Use Cases
          </span>

          <h2 className="text-6xl md:text-7xl lg:text-[128px] font-display tracking-tight leading-[0.9]">
            Manage any
            <br />
            <span className="text-muted-foreground">content type.</span>
          </h2>
        </div>

        {/* Description + Use Cases */}
        <div
          className={`
            transition-all duration-700 delay-100
            ${isVisible ? "opacity-100 translate-y-0" : "opacity-0 translate-y-8"}
            
            /* MOBILE: full width */
            max-w-full
            
            /* DESKTOP: left half */
            lg:max-w-[50%]
          `}
        >
          <p className="text-xl text-muted-foreground mb-12 leading-relaxed max-w-md">
            Whether you&apos;re managing a blog, product catalog, or entire
            documentation site, Velopulent CMS adapts to your content structure
            with flexible schemas and custom fields.
          </p>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-6">
            {useCases.map((useCase, index) => (
              <div
                key={useCase.title}
                className={`transition-all duration-500 ${
                  isVisible
                    ? "opacity-100 translate-y-0"
                    : "opacity-0 translate-y-4"
                }`}
                style={{ transitionDelay: `${index * 50 + 200}ms` }}
              >
                <h3 className="font-medium mb-1">{useCase.title}</h3>
                <p className="text-sm text-muted-foreground">
                  {useCase.description}
                </p>
              </div>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}
