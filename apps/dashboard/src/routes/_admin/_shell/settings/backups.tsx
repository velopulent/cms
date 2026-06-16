import { createFileRoute, redirect } from "@tanstack/react-router";
import { BackupsSection } from "@/components/backups/backups-section";
import { getMe } from "@/lib/api";

export const Route = createFileRoute("/_admin/_shell/settings/backups")({
  beforeLoad: async ({ context }) => {
    const me = await context.queryClient.ensureQueryData({
      queryKey: ["me"],
      queryFn: getMe,
    });
    // Instance-wide backup/restore is owner-only.
    if (me.instance_role !== "instance_owner") {
      throw redirect({ to: "/settings" });
    }
  },
  component: InstanceBackupsSettings,
});

function InstanceBackupsSettings() {
  return <BackupsSection scope={{ kind: "instance" }} />;
}
