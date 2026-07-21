import { createFileRoute, redirect } from "@tanstack/react-router";
import { StorageSettingsPanel } from "@/components/instance/settings-forms";
import { getMe } from "@/lib/api";

export const Route = createFileRoute("/_admin/_shell/settings/storage")({
  beforeLoad: async ({ context }) => {
    const me = await context.queryClient.ensureQueryData({
      queryKey: ["me"],
      queryFn: getMe,
    });
    if (me.instance_role !== "instance_owner")
      throw redirect({ to: "/settings/users" });
  },
  component: StorageSettingsPanel,
});
