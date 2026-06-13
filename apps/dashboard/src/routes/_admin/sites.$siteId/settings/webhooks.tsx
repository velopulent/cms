import { createFileRoute, redirect } from "@tanstack/react-router";
import { WebhooksSection } from "@/components/site-settings/webhooks-section";
import { getSites } from "@/lib/api";

export const Route = createFileRoute("/_admin/sites/$siteId/settings/webhooks")(
  {
    beforeLoad: async ({ context, params }) => {
      const sites = await context.queryClient.ensureQueryData({
        queryKey: ["sites"],
        queryFn: getSites,
      });
      const role = sites.find((s) => s.id === params.siteId)?.role;
      if (role !== "owner" && role !== "admin") {
        throw redirect({
          to: "/sites/$siteId/settings",
          params: { siteId: params.siteId },
        });
      }
    },
    component: WebhooksSettings,
  },
);

function WebhooksSettings() {
  const { siteId } = Route.useParams();
  return <WebhooksSection siteId={siteId} />;
}
