import { createFileRoute, redirect } from "@tanstack/react-router";
import { BackupsSection } from "@/components/backups/backups-section";
import { BackupSettingsPanel } from "@/components/instance/settings-forms";
import { getMe } from "@/lib/api";

export const Route = createFileRoute("/_admin/_shell/settings/backups")({
  beforeLoad: async ({ context }) => {
    const me = await context.queryClient.ensureQueryData({
      queryKey: ["me"],
      queryFn: getMe,
    });
    // Instance-wide backup/restore is owner-only.
    if (me.instance_role !== "instance_owner") {
      throw redirect({ to: "/settings/users" });
    }
  },
  component: InstanceBackupsSettings,
});

function InstanceBackupsSettings() {
  return (
    <div className="space-y-6">
      <BackupSettingsPanel />
      <BackupsSection scope={{ kind: "instance" }} />
    </div>
  );
}
