import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { getMe, instanceRoleLabel } from "@/lib/api";

export const Route = createFileRoute("/_admin/_shell/settings/")({
  component: GeneralInstanceSettings,
});

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-4 py-2">
      <span className="text-sm text-muted-foreground">{label}</span>
      <span className="text-sm font-medium">{value}</span>
    </div>
  );
}

function GeneralInstanceSettings() {
  const { data: me } = useQuery({ queryKey: ["me"], queryFn: getMe });

  return (
    <Card>
      <CardHeader>
        <CardTitle>General</CardTitle>
        <CardDescription>
          Overview of this CMS installation. More instance-wide settings will
          live here.
        </CardDescription>
      </CardHeader>
      <CardContent className="divide-y">
        <Row label="Signed in as" value={me?.username ?? "—"} />
        <Row label="Email" value={me?.email ?? "—"} />
        <Row label="Access" value={instanceRoleLabel(me?.instance_role)} />
      </CardContent>
    </Card>
  );
}
