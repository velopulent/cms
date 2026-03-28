import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { Globe, Plus } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { createSite, getSites } from "@/lib/api";

export const Route = createFileRoute("/_admin/sites/")({
  component: OnboardingPage,
});

function OnboardingPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [name, setName] = useState("");

  const { data: sites, isLoading } = useQuery({
    queryKey: ["sites"],
    queryFn: getSites,
  });

  const createMutation = useMutation({
    mutationFn: () => createSite({ name }),
    onSuccess: (site) => {
      queryClient.invalidateQueries({ queryKey: ["sites"] });
      toast.success("Site created!");
      navigate({
        to: "/sites/$siteId",
        params: { siteId: site.id },
      });
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    createMutation.mutate();
  };

  if (isLoading) {
    return (
      <div className="flex min-h-dvh items-center justify-center p-4">
        <Skeleton className="h-64 w-full max-w-md" />
      </div>
    );
  }

  return (
    <div className="flex min-h-dvh flex-col items-center justify-center gap-8 p-4">
      {sites && sites.length > 0 && (
        <div className="flex w-full max-w-md flex-col gap-4">
          <h2 className="text-lg font-semibold">Your Sites</h2>
          <div className="flex flex-col gap-2">
            {sites.map((site) => (
              <Card
                key={site.id}
                className="cursor-pointer transition-colors hover:bg-muted/50"
                onClick={() =>
                  navigate({
                    to: "/sites/$siteId",
                    params: { siteId: site.id },
                  })
                }
              >
                <CardContent className="flex items-center gap-3 p-4">
                  <Globe className="size-5 text-muted-foreground" />
                  <div className="flex-1">
                    <p className="font-medium">{site.name}</p>
                    <p className="text-xs text-muted-foreground">{site.role}</p>
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        </div>
      )}

      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Plus className="size-5" />
            {sites && sites.length > 0
              ? "Create New Site"
              : "Create Your First Site"}
          </CardTitle>
          <CardDescription>
            {sites && sites.length > 0
              ? "Add another site to your account"
              : "Get started by creating a site to organize your content"}
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={handleSubmit} className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <label htmlFor="site-name" className="text-sm font-medium">
                Site Name
              </label>
              <Input
                id="site-name"
                placeholder="e.g. My Blog"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </div>
            <Button
              type="submit"
              disabled={createMutation.isPending || !name.trim()}
            >
              {createMutation.isPending ? "Creating..." : "Create Site"}
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
