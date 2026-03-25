import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useState, useEffect } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { getSite, updateSite } from "@/lib/api";

export const Route = createFileRoute("/_admin/sites/$siteId/settings")({
  component: SiteSettingsPage,
});

function SiteSettingsPage() {
  const { siteId } = Route.useParams();
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [initialized, setInitialized] = useState(false);

  const { data: site, isLoading } = useQuery({
    queryKey: ["site", siteId],
    queryFn: () => getSite(siteId),
  });

  useEffect(() => {
    if (site && !initialized) {
      setName(site.name);
      setInitialized(true);
    }
  }, [site, initialized]);

  const updateMutation = useMutation({
    mutationFn: () => updateSite(siteId, { name }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["site", siteId] });
      queryClient.invalidateQueries({ queryKey: ["sites"] });
      toast.success("Site settings updated");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    updateMutation.mutate();
  };

  if (isLoading || !initialized) {
    return (
      <div className="flex flex-col gap-6 p-6">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-64 w-full max-w-lg" />
      </div>
    );
  }

  if (!site) {
    return (
      <div className="p-6">
        <p>Site not found.</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6 p-6">
      <div>
        <h1 className="text-2xl font-semibold">Settings</h1>
        <p className="text-sm text-muted-foreground">
          Manage your site settings
        </p>
      </div>

      <form onSubmit={handleSubmit} className="flex flex-col gap-6">
        <Card>
          <CardHeader>
            <CardTitle>General</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="flex flex-col gap-2">
              <label htmlFor="site-name" className="text-sm font-medium">
                Site Name
              </label>
              <Input
                id="site-name"
                placeholder="My Site"
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="max-w-md"
              />
            </div>
          </CardContent>
        </Card>

        <Button
          type="submit"
          className="w-fit"
          disabled={updateMutation.isPending || !name.trim()}
        >
          {updateMutation.isPending ? "Saving..." : "Save Changes"}
        </Button>
      </form>
    </div>
  );
}
