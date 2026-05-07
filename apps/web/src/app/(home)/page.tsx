import { Navigation } from "@/components/landing/navigation";
import { HeroSection } from "@/components/landing/hero-section";
import { FeaturesSection } from "@/components/landing/features-section";
import { HowItWorksSection } from "@/components/landing/how-it-works-section";
import { DatabaseSection } from "@/components/landing/database-section";
import { DashboardSection } from "@/components/landing/dashboard-section";
import { IntegrationsSection } from "@/components/landing/integrations-section";
import { SecuritySection } from "@/components/landing/security-section";
import { UseCasesSection } from "@/components/landing/use-cases";
import { TestimonialsSection } from "@/components/landing/testimonials-section";
import { CtaSection } from "@/components/landing/cta-section";
import { FooterSection } from "@/components/landing/footer-section";
import { BackupSection } from "@/components/landing/backup-section";

export default function Home() {
  return (
    <main className="relative min-h-screen overflow-x-hidden">
      <HeroSection />
      <FeaturesSection />
      <HowItWorksSection />
      <DatabaseSection />
      <BackupSection />
      <DashboardSection />
      <IntegrationsSection />
      <UseCasesSection />
      <SecuritySection />
      {/* <TestimonialsSection /> */}
      <CtaSection />
      <FooterSection />
    </main>
  );
}
