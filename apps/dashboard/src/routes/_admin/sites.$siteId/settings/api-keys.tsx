import { createFileRoute, redirect } from "@tanstack/react-router";
import { ApiKeysSection } from "@/components/site-settings/api-keys-section";
import { getMe, isOperator } from "@/lib/api";

export const Route = createFileRoute("/_admin/sites/$siteId/settings/api-keys")(
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
    component: ApiKeysSettings,
  },
);

function ApiKeysSettings() {
  const { siteId } = Route.useParams();
  return <ApiKeysSection siteId={siteId} />;
}
