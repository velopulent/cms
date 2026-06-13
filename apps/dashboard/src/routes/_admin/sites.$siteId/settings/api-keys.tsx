import { createFileRoute, redirect } from "@tanstack/react-router";
import { ApiKeysSection } from "@/components/site-settings/api-keys-section";
import { getSites } from "@/lib/api";

export const Route = createFileRoute("/_admin/sites/$siteId/settings/api-keys")(
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
    component: ApiKeysSettings,
  },
);

function ApiKeysSettings() {
  const { siteId } = Route.useParams();
  return <ApiKeysSection siteId={siteId} />;
}
