import { BackupSection } from "@/components/landing/backup-section";
import { CtaSection } from "@/components/landing/cta-section";
import { DashboardSection } from "@/components/landing/dashboard-section";
import { DatabaseSection } from "@/components/landing/database-section";
import { FeaturesSection } from "@/components/landing/features-section";
import { FooterSection } from "@/components/landing/footer-section";
import { HeroSection } from "@/components/landing/hero-section";
import { HowItWorksSection } from "@/components/landing/how-it-works-section";
import { IntegrationsSection } from "@/components/landing/integrations-section";
import { Navigation } from "@/components/landing/navigation";
import { SecuritySection } from "@/components/landing/security-section";
import { TestimonialsSection } from "@/components/landing/testimonials-section";
import { UseCasesSection } from "@/components/landing/use-cases";

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
