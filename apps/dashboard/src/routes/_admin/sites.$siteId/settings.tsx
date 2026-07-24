import {
  createFileRoute,
  Link,
  Outlet,
  useRouterState,
} from "@tanstack/react-router";
import { useSiteRole } from "@/components/site-settings/use-site-role";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";

export const Route = createFileRoute("/_admin/sites/$siteId/settings")({
  component: SettingsLayout,
});

function SettingsLayout() {
  const { siteId } = Route.useParams();
  const { canManage } = useSiteRole(siteId);
  const pathname = useRouterState({ select: (s) => s.location.pathname });

  const base = `/sites/${siteId}/settings`;
  const active = pathname.startsWith(`${base}/`)
    ? pathname.slice(base.length + 1).split("/")[0]
    : "general";

  return (
    <div className="flex flex-col gap-6 p-4 sm:p-6 w-full max-w-4xl mx-auto">
      <div>
        <h1 className="text-2xl font-semibold">Settings</h1>
        <p className="text-sm text-muted-foreground">
          Manage your site settings
        </p>
      </div>

      <Tabs value={active}>
        <TabsList>
          {canManage && (
            <TabsTrigger
              value="general"
              nativeButton={false}
              render={<Link to="/sites/$siteId/settings" params={{ siteId }} />}
            >
              General
            </TabsTrigger>
          )}
          <TabsTrigger
            value="mcp"
            nativeButton={false}
            render={
              <Link to="/sites/$siteId/settings/mcp" params={{ siteId }} />
            }
          >
            MCP
          </TabsTrigger>
          <TabsTrigger
            value="members"
            nativeButton={false}
            render={
              <Link to="/sites/$siteId/settings/members" params={{ siteId }} />
            }
          >
            Members
          </TabsTrigger>
          {canManage && (
            <TabsTrigger
              value="api-keys"
              nativeButton={false}
              render={
                <Link
                  to="/sites/$siteId/settings/api-keys"
                  params={{ siteId }}
                />
              }
            >
              API Keys
            </TabsTrigger>
          )}
          {canManage && (
            <TabsTrigger
              value="webhooks"
              nativeButton={false}
              render={
                <Link
                  to="/sites/$siteId/settings/webhooks"
                  params={{ siteId }}
                />
              }
            >
              Webhooks
            </TabsTrigger>
          )}
          {canManage && (
            <TabsTrigger
              value="backups"
              nativeButton={false}
              render={
                <Link
                  to="/sites/$siteId/settings/backups"
                  params={{ siteId }}
                />
              }
            >
              Backups
            </TabsTrigger>
          )}
        </TabsList>
      </Tabs>

      <Outlet />
    </div>
  );
}
