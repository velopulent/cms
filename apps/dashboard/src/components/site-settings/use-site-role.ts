import { useQuery } from "@tanstack/react-query";
import { getMe, getSites, isOperator } from "@/lib/api";

export type SiteRole = "editor" | "viewer";

/**
 * Resolve the current user's effective authority for a site.
 * Site management (schema, keys, webhooks, members, settings) is performed by
 * instance operators (owner/admin), so `canManage` is driven by the instance
 * role; `role` reflects the per-site collaborator role for non-operators.
 */
export function useSiteRole(siteId: string) {
  const { data: me } = useQuery({ queryKey: ["me"], queryFn: getMe });
  const { data: sites } = useQuery({ queryKey: ["sites"], queryFn: getSites });

  const operator = isOperator(me?.instance_role);
  const siteRole = sites?.find((item) => item.id === siteId)?.role;
  const role: SiteRole = siteRole === "editor" ? "editor" : "viewer";

  return { role, canManage: operator, isOperator: operator };
}
