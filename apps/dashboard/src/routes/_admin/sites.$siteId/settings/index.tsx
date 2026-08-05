import { createFileRoute } from "@tanstack/react-router";
import { GeneralSection } from "@/components/site-settings/general-section";
import { useSiteRole } from "@/components/site-settings/use-site-role";

export const Route = createFileRoute("/_admin/sites/$siteId/settings/")({
  component: GeneralSettings,
});

function GeneralSettings() {
  const { siteId } = Route.useParams();
  const { canManage, role } = useSiteRole(siteId);
  return <GeneralSection siteId={siteId} canManage={canManage} role={role} />;
}
