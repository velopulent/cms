"use client";

import { Icon } from "@iconify-icon/react";
import { Check, Code2, Download } from "lucide-react";
import Image from "next/image";
import { useState } from "react";
import { FooterSection } from "@/components/landing/footer-section";
import { Navigation } from "@/components/landing/navigation";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

type OS = "linux" | "macos" | "windows";
type Architecture = "x86_64" | "aarch64" | "arm64";

interface ReleaseInfo {
  os: OS;
  arch: Architecture;
  filename: string;
  size: string;
  sha256: string;
}

const releases: ReleaseInfo[] = [
  {
    os: "linux",
    arch: "x86_64",
    filename: "velopulent-cms-linux-x86_64",
    size: "12.5 MB",
    sha256: "abc123...",
  },
  {
    os: "linux",
    arch: "aarch64",
    filename: "velopulent-cms-linux-aarch64",
    size: "11.8 MB",
    sha256: "def456...",
  },
  {
    os: "macos",
    arch: "x86_64",
    filename: "velopulent-cms-macos-x86_64",
    size: "13.2 MB",
    sha256: "ghi789...",
  },
  {
    os: "macos",
    arch: "aarch64",
    filename: "velopulent-cms-macos-aarch64",
    size: "12.1 MB",
    sha256: "jkl012...",
  },
  {
    os: "windows",
    arch: "x86_64",
    filename: "velopulent-cms-windows-x86_64.exe",
    size: "14.5 MB",
    sha256: "mno345...",
  },
];

const osOptions: { value: OS; label: string; icon: string }[] = [
  { value: "linux", label: "Linux", icon: "logos:linux-tux" },
  { value: "macos", label: "macOS", icon: "lineicons:apple-brand" },
  { value: "windows", label: "Windows", icon: "logos:microsoft-windows-icon" },
];

const archOptions: { value: Architecture; label: string }[] = [
  { value: "x86_64", label: "x86_64 (Intel/AMD)" },
  { value: "aarch64", label: "ARM64" },
  { value: "arm64", label: "ARM64 (Apple Silicon)" },
];

const features = [
  {
    icon: Download,
    title: "Single Binary",
    description:
      "Download once, run anywhere. No dependencies or installation required.",
  },
  {
    icon: Code2,
    title: "Open Source",
    description:
      "100% open source under AGPL v3. Audit the code, customize as needed.",
  },
  {
    icon: Check,
    title: "Verified Checksums",
    description:
      "Every release is signed and verified for security and integrity.",
  },
];

export default function DownloadPage() {
  const [selectedOS, setSelectedOS] = useState<OS>("linux");
  const [selectedArch, setSelectedArch] = useState<Architecture>("x86_64");

  const selectedRelease = releases.find(
    (r) => r.os === selectedOS && r.arch === selectedArch,
  );

  const getArchOptions = () => {
    if (selectedOS === "windows") {
      return archOptions.slice(0, 1);
    }
    if (selectedOS === "macos") {
      return archOptions.slice(1);
    }
    return archOptions.slice(0, 2);
  };

  const currentArchOptions = getArchOptions();
  if (
    selectedArch &&
    !currentArchOptions.some((a) => a.value === selectedArch)
  ) {
    setSelectedArch(currentArchOptions[0].value);
  }

  return (
    <>
      <Navigation />
      <main className="min-h-screen bg-background">
        {/* Hero Section */}
        <section className="relative overflow-hidden pt-32 pb-20 px-4 sm:px-6 lg:px-8">
          <div className="max-w-7xl mx-auto">
            {/* Background Elements */}
            <div className="absolute inset-0 -z-10">
              <div className="absolute top-1/2 left-0 w-96 h-96 bg-foreground/5 rounded-full blur-3xl opacity-0 group-hover:opacity-100 transition-opacity duration-700" />
              <div className="absolute top-1/3 right-0 w-80 h-80 bg-accent/5 rounded-full blur-3xl" />
            </div>

            <div className="grid lg:grid-cols-2 gap-12 lg:gap-16 items-center">
              {/* Left: Heading */}
              <div className="space-y-8">
                <div className="space-y-4">
                  <h1 className="text-5xl md:text-6xl lg:text-7xl font-display tracking-tight leading-[0.95]">
                    Download Velopulent CMS
                  </h1>
                  <p className="text-xl text-muted-foreground leading-relaxed max-w-xl">
                    Get the latest version of Velopulent CMS. Choose your
                    operating system and architecture below to download the
                    binary that works for you.
                  </p>
                </div>

                {/* Stats */}
                <div className="grid grid-cols-2 gap-4 pt-4">
                  <div className="border border-foreground/10 p-4 rounded-sm">
                    <div className="text-3xl font-display">100%</div>
                    <div className="text-sm text-muted-foreground">
                      Open Source
                    </div>
                  </div>
                  <div className="border border-foreground/10 p-4 rounded-sm">
                    <div className="text-3xl font-display">&lt;20MB</div>
                    <div className="text-sm text-muted-foreground">
                      Binary Size
                    </div>
                  </div>
                </div>
              </div>

              <img
                src={"/assets/logo.webp"}
                alt="Velopulent CMS Logo"
                width={620}
                height={420}
                className="relative"
              />
            </div>
          </div>
        </section>

        {/* Download Section */}
        <section className="py-20 px-4 sm:px-6 lg:px-8 bg-secondary/20">
          <div className="max-w-4xl mx-auto">
            <div className="mb-12 text-center lg:text-left">
              <h2 className="text-3xl font-display mb-4">
                Select Your Platform
              </h2>
              <p className="text-muted-foreground max-w-2xl">
                Choose your operating system and architecture. Don&apos;t know
                which one? We&apos;ll help you find the right download.
              </p>
            </div>

            {/* Selection Grid */}
            <div className="space-y-8 mb-12">
              {/* OS Selection */}
              <div>
                <label className="block text-sm font-medium mb-4">
                  Operating System
                </label>
                <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
                  {osOptions.map((option) => (
                    <button
                      key={option.value}
                      onClick={() => setSelectedOS(option.value)}
                      className={`relative p-4 rounded-lg border-2 transition-all duration-200 text-left ${
                        selectedOS === option.value
                          ? "border-foreground bg-foreground/5"
                          : "border-foreground/20 hover:border-foreground/40"
                      }`}
                    >
                      <div className="flex items-center gap-3">
                        <Icon icon={option.icon} />
                        <div>
                          <div className="font-medium">{option.label}</div>
                          <div className="text-xs text-muted-foreground">
                            {option.value === "linux" && "Intel, AMD"}
                            {option.value === "macos" && "Intel, Apple Silicon"}
                            {option.value === "windows" && "Intel, AMD"}
                          </div>
                        </div>
                      </div>
                      {selectedOS === option.value && (
                        <div className="absolute top-3 right-3 w-5 h-5 bg-foreground rounded-full flex items-center justify-center">
                          <Check className="w-3 h-3 text-background" />
                        </div>
                      )}
                    </button>
                  ))}
                </div>
              </div>

              {/* Architecture Selection */}
              <div>
                <label className="block text-sm font-medium mb-4">
                  Architecture
                </label>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 max-w-lg">
                  {currentArchOptions.map((option) => (
                    <button
                      key={option.value}
                      onClick={() => setSelectedArch(option.value)}
                      className={`relative p-4 rounded-lg border-2 transition-all duration-200 text-left ${
                        selectedArch === option.value
                          ? "border-foreground bg-foreground/5"
                          : "border-foreground/20 hover:border-foreground/40"
                      }`}
                    >
                      <div className="font-medium">{option.label}</div>
                      {selectedArch === option.value && (
                        <div className="absolute top-3 right-3 w-5 h-5 bg-foreground rounded-full flex items-center justify-center">
                          <Check className="w-3 h-3 text-background" />
                        </div>
                      )}
                    </button>
                  ))}
                </div>
              </div>
            </div>

            {/* Download Card */}
            {selectedRelease && (
              <div className="bg-card border border-foreground/10 rounded-lg p-8 space-y-6">
                <div className="space-y-2">
                  <h3 className="text-xl font-semibold">Ready to Download</h3>
                  <p className="text-muted-foreground">
                    {selectedOS === "linux" && "For Linux systems"}
                    {selectedOS === "macos" && "For macOS systems"}
                    {selectedOS === "windows" && "For Windows systems"} with{" "}
                    {selectedArch === "x86_64"
                      ? "Intel/AMD"
                      : selectedArch === "aarch64"
                        ? "ARM64"
                        : "Apple Silicon"}{" "}
                    architecture
                  </p>
                </div>

                <div className="border-t border-foreground/10 pt-6 grid sm:grid-cols-3 gap-6">
                  <div>
                    <div className="text-xs text-muted-foreground mb-1">
                      Filename
                    </div>
                    <div className="font-mono text-sm break-all">
                      {selectedRelease.filename}
                    </div>
                  </div>
                  <div>
                    <div className="text-xs text-muted-foreground mb-1">
                      Size
                    </div>
                    <div className="font-mono text-sm">
                      {selectedRelease.size}
                    </div>
                  </div>
                  <div>
                    <div className="text-xs text-muted-foreground mb-1">
                      SHA256
                    </div>
                    <div className="font-mono text-xs text-foreground/60 truncate">
                      {selectedRelease.sha256}
                    </div>
                  </div>
                </div>

                <div className="flex flex-col sm:flex-row gap-4 pt-4">
                  <Button className="flex-1 h-12 text-base rounded-full bg-foreground text-background hover:bg-foreground/90">
                    <Download className="w-4 h-4 mr-2" />
                    Download Version 1.0.0
                  </Button>
                  <Button
                    variant="outline"
                    className="flex-1 h-12 text-base rounded-full"
                  >
                    View Release Notes
                  </Button>
                </div>

                <div className="bg-muted/30 rounded-lg p-4 border border-foreground/5">
                  <p className="text-xs text-muted-foreground leading-relaxed">
                    By downloading, you agree to the{" "}
                    <a href="#" className="text-foreground hover:underline">
                      AGPL v3 License
                    </a>
                    . All downloads are verified with SHA256 checksums. Need
                    help? Check our{" "}
                    <a href="#" className="text-foreground hover:underline">
                      installation guide
                    </a>
                    .
                  </p>
                </div>
              </div>
            )}
          </div>
        </section>

        {/* Features Section */}
        <section className="py-20 px-4 sm:px-6 lg:px-8">
          <div className="max-w-4xl mx-auto">
            <h2 className="text-3xl font-display mb-12 text-center">
              Why Download Velopulent
            </h2>
            <div className="grid md:grid-cols-3 gap-8">
              {features.map((feature) => {
                const Icon = feature.icon;
                return (
                  <div key={feature.title} className="text-center space-y-4">
                    <div className="w-12 h-12 bg-foreground/10 rounded-lg flex items-center justify-center mx-auto">
                      <Icon className="w-6 h-6 text-foreground" />
                    </div>
                    <h3 className="font-semibold">{feature.title}</h3>
                    <p className="text-sm text-muted-foreground">
                      {feature.description}
                    </p>
                  </div>
                );
              })}
            </div>
          </div>
        </section>

        {/* Alternative Download Methods */}
        <section className="py-20 px-4 sm:px-6 lg:px-8 bg-secondary/20">
          <div className="max-w-4xl mx-auto">
            <h2 className="text-3xl font-display mb-4">
              Alternative Download Methods
            </h2>
            <p className="text-muted-foreground mb-8">
              Prefer command line? Use these methods to get started quickly.
            </p>

            <div className="grid md:grid-cols-2 gap-6">
              <div className="bg-card border border-foreground/10 rounded-lg p-6 space-y-4">
                <h3 className="font-semibold flex items-center gap-2">
                  <Code2 className="w-5 h-5" />
                  From Source
                </h3>
                <pre className="bg-background rounded p-4 text-sm overflow-x-auto font-mono text-muted-foreground">
                  <code>{`git clone https://github.com/velopulent/cms.git
cd cms
cargo build --release`}</code>
                </pre>
              </div>

              <div className="bg-card border border-foreground/10 rounded-lg p-6 space-y-4">
                <h3 className="font-semibold flex items-center gap-2">
                  <Code2 className="w-5 h-5" />
                  Docker
                </h3>
                <pre className="bg-background rounded p-4 text-sm overflow-x-auto font-mono text-muted-foreground">
                  <code>{`docker pull velopulent/cms:latest
docker run -p 3000:3000 velopulent/cms`}</code>
                </pre>
              </div>
            </div>
          </div>
        </section>

        {/* Support Section */}
        <section className="py-20 px-4 sm:px-6 lg:px-8">
          <div className="max-w-4xl mx-auto">
            <div className="bg-card border border-foreground/10 rounded-lg p-8 sm:p-12 text-center space-y-6">
              <h2 className="text-3xl font-display">
                Getting Started with Velopulent CMS
              </h2>
              <p className="text-muted-foreground text-lg max-w-2xl mx-auto">
                After downloading, check out our documentation to set up your
                first project and start managing content.
              </p>
              <div className="flex flex-col sm:flex-row gap-4 justify-center pt-4">
                <Button className="px-8 h-12 rounded-full bg-foreground text-background hover:bg-foreground/90">
                  Read Documentation
                </Button>
                <Button variant="outline" className="px-8 h-12 rounded-full">
                  Join Community
                </Button>
              </div>
            </div>
          </div>
        </section>
      </main>
      <FooterSection />
    </>
  );
}
