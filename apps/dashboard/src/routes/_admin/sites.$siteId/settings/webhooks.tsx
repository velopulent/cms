import { createFileRoute, redirect } from "@tanstack/react-router";
import { WebhooksSection } from "@/components/site-settings/webhooks-section";
import { getMe, isOperator } from "@/lib/api";

export const Route = createFileRoute("/_admin/sites/$siteId/settings/webhooks")(
  {
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
    component: WebhooksSettings,
  },
);

function WebhooksSettings() {
  const { siteId } = Route.useParams();
  return <WebhooksSection siteId={siteId} />;
}
