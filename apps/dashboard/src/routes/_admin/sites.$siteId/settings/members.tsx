import { createFileRoute } from "@tanstack/react-router";
import { MembersSection } from "@/components/site-settings/members-section";
import { useSiteRole } from "@/components/site-settings/use-site-role";

export const Route = createFileRoute("/_admin/sites/$siteId/settings/members")({
  component: MembersSettings,
});

function MembersSettings() {
  const { siteId } = Route.useParams();
  const { canManage } = useSiteRole(siteId);
  return <MembersSection siteId={siteId} canManage={canManage} />;
}
