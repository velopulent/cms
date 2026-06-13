import { useQuery } from "@tanstack/react-query";
import { getSites } from "@/lib/api";

export type SiteRole = "owner" | "admin" | "editor" | "viewer";

/** Resolve the current user's role for a site from the cached sites list. */
export function useSiteRole(siteId: string) {
  const { data: sites } = useQuery({
    queryKey: ["sites"],
    queryFn: getSites,
  });
  const role = (sites?.find((item) => item.id === siteId)?.role ??
    "viewer") as SiteRole;
  return { role, canManage: role === "owner" || role === "admin" };
}
