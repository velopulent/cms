import { createFileRoute, redirect } from "@tanstack/react-router";
import { getSites } from "@/lib/api";

export const Route = createFileRoute("/_admin/")({
  beforeLoad: async () => {
    try {
      const sites = await getSites();
      if (sites.length > 0) {
        throw redirect({
          to: "/sites/$siteId",
          params: { siteId: sites[0].id },
        });
      } else {
        throw redirect({ to: "/sites" });
      }
    } catch (e: any) {
      if (e?.redirect) throw e;
      throw redirect({ to: "/sites" });
    }
  },
  component: () => null,
});
