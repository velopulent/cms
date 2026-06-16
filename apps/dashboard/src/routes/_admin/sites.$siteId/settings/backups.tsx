import { createFileRoute, redirect } from "@tanstack/react-router";
import { BackupsSection } from "@/components/backups/backups-section";
import { getMe, isOperator } from "@/lib/api";

export const Route = createFileRoute("/_admin/sites/$siteId/settings/backups")({
  beforeLoad: async ({ context, params }) => {
    const me = await context.queryClient.ensureQueryData({
      queryKey: ["me"],
      queryFn: getMe,
    });
    if (!isOperator(me.instance_role)) {
      throw redirect({
        to: "/sites/$siteId/settings",
        params: { siteId: params.siteId },
      });
    }
  },
  component: SiteBackupsSettings,
});

function SiteBackupsSettings() {
  const { siteId } = Route.useParams();
  return <BackupsSection scope={{ kind: "site", siteId }} />;
}
