"use client";

import { useEffect, useRef, useState } from "react";

const dashboardCards = [
  {
    eyebrow: "Workspaces",
    title: "Manage multiple sites",
    description:
      "Switch between projects, teams, and environments from one dashboard.",
  },
  {
    eyebrow: "Progressive Web App",
    title: "Install anywhere",
    description:
      "Use Velopulent CMS across desktop and mobile with offline support and native-like experience.",
  },
  {
    eyebrow: "Rich Text Editor",
    title: "Write without distractions",
    description:
      "Powerful rich text editing with markdown support and live preview.",
  },
];

function GridBackground() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const timeRef = useRef(0);
  const frameRef = useRef(0);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      canvas.width = rect.width * dpr;
      canvas.height = rect.height * dpr;
      ctx.scale(dpr, dpr);
    };
    resize();
    window.addEventListener("resize", resize);

    const render = () => {
      const rect = canvas.getBoundingClientRect();
      const width = rect.width;
      const height = rect.height;
      ctx.clearRect(0, 0, width, height);
      const gridSize = 60;
      const time = timeRef.current;
      for (let x = 0; x < width; x += gridSize) {
        for (let y = 0; y < height; y += gridSize) {
          const wave = Math.sin(x * 0.01 + y * 0.01 + time) * 0.5 + 0.5;
          const size = 1 + wave * 2;
          ctx.beginPath();
          ctx.arc(x, y, size, 0, Math.PI * 2);
          ctx.fillStyle = "rgba(255, 255, 255, 0.04)";
          ctx.fill();
        }
      }
      const pulseY = (time * 30) % height;
      ctx.strokeStyle = "rgba(255, 255, 255, 0.03)";
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(0, pulseY);
      ctx.lineTo(width, pulseY);
      ctx.stroke();
      timeRef.current += 0.02;
      frameRef.current = requestAnimationFrame(render);
    };
    render();

    return () => {
      window.removeEventListener("resize", resize);
      cancelAnimationFrame(frameRef.current);
    };
  }, []);

  return (
    <canvas
      ref={canvasRef}
      className="absolute inset-0 pointer-events-none"
      style={{ width: "100%", height: "100%" }}
    />
  );
}

function DotGraph({
  color = "white",
  height = 32,
  freq1 = 0.35,
  freq2 = 0.12,
  freqT = 0.7,
  speed = 0.025,
  baseline = 0.3,
  amplitude = 0.5,
}: {
  color?: string;
  height?: number;
  freq1?: number;
  freq2?: number;
  freqT?: number;
  speed?: number;
  baseline?: number;
  amplitude?: number;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const frameRef = useRef(0);
  const timeRef = useRef(Math.random() * 100);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const W = canvas.offsetWidth || 300;
    const H = height;
    canvas.width = W * dpr;
    canvas.height = H * dpr;
    ctx.scale(dpr, dpr);

    const render = () => {
      ctx.clearRect(0, 0, W, H);
      const t = timeRef.current;
      const cols = Math.floor(W / 8);

      for (let i = 0; i < cols; i++) {
        const raw =
          baseline +
          amplitude * Math.sin(i * freq1 + t) * Math.cos(i * freq2 + t * freqT);
        const v = Math.max(0, Math.min(1, raw));
        const dotY = H - 4 - v * (H - 8);
        const x = i * 8 + 4;
        const alpha = 0.15 + v * 0.55;
        const r = 1.5 + v * 1.2;

        ctx.beginPath();
        ctx.arc(x, dotY, r, 0, Math.PI * 2);
        ctx.fillStyle =
          color === "green"
            ? `rgba(236, 168, 214, ${alpha})`
            : `rgba(255, 255, 255, ${alpha})`;
        ctx.fill();
      }

      timeRef.current += speed;
      frameRef.current = requestAnimationFrame(render);
    };

    render();
    return () => cancelAnimationFrame(frameRef.current);
  }, [color, height, freq1, freq2, freqT, speed, baseline, amplitude]);

  return (
    <canvas
      ref={canvasRef}
      style={{ width: "100%", height: `${height}px`, display: "block" }}
    />
  );
}

export function DashboardSection() {
  const [time, setTime] = useState<Date | null>(null);
  const [isVisible, setIsVisible] = useState(false);
  const sectionRef = useRef<HTMLElement>(null);

  useEffect(() => {
    setTime(new Date());
    const interval = setInterval(() => setTime(new Date()), 1000);
    return () => clearInterval(interval);
  }, []);

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
      ref={sectionRef}
      className="relative py-32 lg:py-40 overflow-hidden"
    >
      <GridBackground />

      <div className="relative z-10 max-w-350 mx-auto px-6 lg:px-12">
        {/* Header */}
        <div className="grid lg:grid-cols-12 gap-8 mb-20 lg:mb-32">
          <div className="lg:col-span-8 lg:col-start-1">
            <div className="flex items-center gap-4 mb-6">
              <span className="flex items-center gap-2 px-3 py-1 bg-[#eca8d6]/10 text-[#eca8d6] text-xs font-mono">
                <span className="w-2 h-2 rounded-full bg-[#eca8d6] animate-pulse" />
                LIVE
              </span>
              <span className="text-sm font-mono text-muted-foreground">
                {time ? `${time.toLocaleTimeString("en-GB")} UTC` : ""}
              </span>
            </div>

            <h2
              className={`text-6xl md:text-7xl lg:text-[140px] font-display tracking-tight leading-[0.95] transition-all duration-1000 ${
                isVisible
                  ? "opacity-100 translate-y-0"
                  : "opacity-0 translate-y-8"
              }`}
            >
              Beautiful
              <br />
              <span className="text-muted-foreground">Dashboard.</span>
            </h2>
          </div>
        </div>

        {/* Organic graph image */}
        <div
          className={`w-full mb-0 transition-all duration-1000 delay-200 ${
            isVisible ? "opacity-100" : "opacity-0"
          }`}
        >
          <img
            src="/assets/real-time-graph.png"
            alt=""
            aria-hidden="true"
            className="w-full h-auto object-cover"
          />
        </div>

        {/* Metrics grid */}
        <div className="grid lg:grid-cols-3 gap-6">
          {/* Large metric */}
          <div
            className={`lg:col-span-1 bg-foreground/2 border border-foreground/10 p-10 lg:p-14 transition-all duration-700 ${
              isVisible
                ? "opacity-100 translate-y-0"
                : "opacity-0 translate-y-12"
            }`}
          >
            <div className="text-4xl md:text-5xl font-display tracking-tight mb-4 whitespace-nowrap overflow-hidden">
              {dashboardCards[0].eyebrow}
            </div>
            <div className="mb-6">
              <DotGraph
                color="white"
                height={36}
                freq1={0.28}
                freq2={0.09}
                freqT={0.5}
                speed={0.018}
                baseline={0.35}
                amplitude={0.55}
              />
            </div>
            <div className="text-lg text-foreground mb-2">
              {dashboardCards[0].title}
            </div>
            <div className="text-sm text-muted-foreground font-mono">
              {dashboardCards[0].description}
            </div>
          </div>

          {/* Metrics */}
          {dashboardCards.slice(1).map((item, index) => (
            <div
              key={item.eyebrow}
              className={`bg-foreground/2 border border-foreground/10 p-8 flex flex-col items-start justify-between gap-6 transition-all duration-700 ${
                isVisible
                  ? "opacity-100 translate-y-0"
                  : "opacity-0 translate-y-12"
              }`}
              style={{ transitionDelay: `${(index + 1) * 100}ms` }}
            >
              <div className="w-full">
                <div className="text-sm text-muted-foreground font-mono mb-2">
                  {item.description}
                </div>
                <div className="text-base text-foreground mb-3">
                  {item.title}
                </div>
                <DotGraph
                  color={index === 0 ? "green" : "white"}
                  height={24}
                  freq1={index === 0 ? 0.45 : 0.22}
                  freq2={index === 0 ? 0.18 : 0.07}
                  freqT={index === 0 ? 1.1 : 0.4}
                  speed={index === 0 ? 0.032 : 0.015}
                  baseline={index === 0 ? 0.4 : 0.25}
                  amplitude={index === 0 ? 0.45 : 0.6}
                />
              </div>
              <div className="text-3xl md:text-4xl font-display tracking-tight w-full">
                {item.eyebrow}
              </div>
            </div>
          ))}
        </div>

        {/* Bottom ticker */}
        <div
          className={`mt-16 pt-8 border-t border-foreground/10 flex flex-wrap items-center gap-x-12 gap-y-4 text-sm font-mono text-muted-foreground transition-all duration-1000 delay-500 ${
            isVisible ? "opacity-100" : "opacity-0"
          }`}
        >
          <span>Intuitive UI</span>
          <span>Rich text editor</span>
          <span>Media library</span>
          <span>Version history</span>
          <span className="text-foreground">Embedded</span>
        </div>
      </div>
    </section>
  );
}
