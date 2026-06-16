import { useQuery } from "@tanstack/react-query";
import {
  createFileRoute,
  Link,
  Outlet,
  redirect,
  useRouterState,
} from "@tanstack/react-router";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { getMe, isOperator } from "@/lib/api";

export const Route = createFileRoute("/_admin/_shell/settings")({
  beforeLoad: async ({ context }) => {
    const user = await context.queryClient.ensureQueryData({
      queryKey: ["me"],
      queryFn: getMe,
    });
    if (!isOperator(user.instance_role)) {
      throw redirect({ to: "/" });
    }
  },
  component: InstanceSettingsLayout,
});

function InstanceSettingsLayout() {
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const { data: me } = useQuery({ queryKey: ["me"], queryFn: getMe });
  const isOwner = me?.instance_role === "instance_owner";
  const active = pathname.endsWith("/settings/users")
    ? "users"
    : pathname.endsWith("/settings/backups")
      ? "backups"
      : "general";

  return (
    <main className="container mx-auto flex w-full max-w-5xl flex-col gap-6 p-4 sm:p-6">
      <div>
        <h1 className="text-2xl font-semibold">Instance settings</h1>
        <p className="text-sm text-muted-foreground">
          Manage installation-wide configuration and users.
        </p>
      </div>

      <Tabs value={active}>
          <TabsList>
            <TabsTrigger
              value="general"
              nativeButton={false}
              render={<Link to="/settings" />}
            >
              General
            </TabsTrigger>
            <TabsTrigger
              value="users"
              nativeButton={false}
              render={<Link to="/settings/users" />}
            >
              Users
            </TabsTrigger>
            {isOwner && (
              <TabsTrigger
                value="backups"
                nativeButton={false}
                render={<Link to="/settings/backups" />}
              >
                Backups
              </TabsTrigger>
            )}
          </TabsList>
      </Tabs>

      <Outlet />
    </main>
  );
}
