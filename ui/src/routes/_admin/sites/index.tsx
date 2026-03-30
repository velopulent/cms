import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  createFileRoute,
  useNavigate,
  useSearch,
} from "@tanstack/react-router";
import { Cloud, Globe, HardDrive, Plus } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { z } from "zod";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { createSite, getSites, type SiteWithRole } from "@/lib/api";

export const Route = createFileRoute("/_admin/sites/")({
  validateSearch: z.object({
    create: z.boolean().optional(),
  }),
  component: SitesPage,
});

function formatDate(dateStr: string) {
  const date = new Date(dateStr);
  return date.toLocaleDateString("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

function SiteCard({ site }: { site: SiteWithRole }) {
  const navigate = useNavigate();

  const roleColors = {
    owner: "bg-purple-500/10 text-purple-700 border-purple-500/20",
    admin: "bg-blue-500/10 text-blue-700 border-blue-500/20",
    editor: "bg-green-500/10 text-green-700 border-green-500/20",
    viewer: "bg-orange-500/10 text-orange-700 border-orange-500/20",
  } as const;

  return (
    <Card
      className="cursor-pointer transition-all hover:border-primary/50 hover:shadow-md"
      onClick={() =>
        navigate({
          to: "/sites/$siteId",
          params: { siteId: site.id },
        })
      }
    >
      <CardHeader className="pb-3">
        <div className="flex items-start justify-between">
          <div className="flex items-center gap-3">
            <div className="flex size-10 items-center justify-center rounded-lg bg-primary/10">
              <Globe className="size-5 text-primary" />
            </div>
            <div>
              <CardTitle className="text-lg font-semibold">
                {site.name}
              </CardTitle>
              <CardDescription className="text-xs">
                Created {formatDate(site.created_at)}
              </CardDescription>
            </div>
          </div>
        </div>
      </CardHeader>
      <CardContent className="flex items-center justify-between pt-0">
        <Badge
          variant="outline"
          className={
            roleColors[site.role as keyof typeof roleColors] || "bg-muted"
          }
        >
          {site.role}
        </Badge>
      </CardContent>
    </Card>
  );
}

function CreateSiteDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [storageProvider, setStorageProvider] = useState("filesystem");

  const createMutation = useMutation({
    mutationFn: () =>
      createSite({
        name,
        default_storage_provider: storageProvider,
      }),
    onSuccess: (site) => {
      queryClient.invalidateQueries({ queryKey: ["sites"] });
      toast.success("Site created!");
      setName("");
      setStorageProvider("filesystem");
      onOpenChange(false);
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

  useEffect(() => {
    if (!open) {
      setName("");
      setStorageProvider("filesystem");
    }
  }, [open]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Create New Site</DialogTitle>
          <DialogDescription>
            Set up a new site to organize your content
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="flex flex-col gap-6">
          <div className="flex flex-col gap-2">
            <Label htmlFor="site-name">Site Name</Label>
            <Input
              id="site-name"
              placeholder="e.g. My Portfolio"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>

          <div className="flex flex-col gap-2">
            <Label htmlFor="storage-provider">File Storage</Label>
            <Select
              value={storageProvider}
              onValueChange={(v) => v && setStorageProvider(v)}
            >
              <SelectTrigger id="storage-provider" className={"w-52"}>
                {storageProvider === "filesystem" ? (
                  <div className="flex items-center gap-2">
                    <HardDrive className="size-4" />
                    <span>Filesystem</span>
                  </div>
                ) : storageProvider === "s3" ? (
                  <div className="flex items-center gap-2">
                    <Cloud className="size-4" />
                    <span>S3 / Cloud Storage</span>
                  </div>
                ) : (
                  <SelectValue placeholder="Select storage type" />
                )}
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="filesystem">
                  <div className="flex items-center gap-2">
                    <HardDrive className="size-4" />
                    <span>Filesystem</span>
                    <span className="text-xs text-muted-foreground">
                      (default)
                    </span>
                  </div>
                </SelectItem>
                <SelectItem value="s3">
                  <div className="flex items-center gap-2">
                    <Cloud className="size-4" />
                    <span>S3 / Cloud Storage</span>
                  </div>
                </SelectItem>
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground">
              {storageProvider === "s3"
                ? "Files will be stored in your S3 bucket"
                : "Files will be stored on the local filesystem"}
            </p>
          </div>

          <div className="flex justify-end gap-2">
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button
              type="submit"
              disabled={createMutation.isPending || !name.trim()}
            >
              {createMutation.isPending ? "Creating..." : "Create Site"}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function SitesPage() {
  const navigate = useNavigate();
  const search = useSearch({ from: "/_admin/sites/" });
  const [createOpen, setCreateOpen] = useState(false);

  const { data: sites, isLoading } = useQuery({
    queryKey: ["sites"],
    queryFn: getSites,
  });

  useEffect(() => {
    if (search.create) {
      setCreateOpen(true);
      navigate({ to: "/sites", search: {}, replace: true });
    }
  }, [search.create, navigate]);

  if (isLoading) {
    return (
      <div className="container mx-auto max-w-5xl p-6">
        <div className="mb-6 flex items-center justify-between">
          <div>
            <Skeleton className="h-8 w-24" />
            <Skeleton className="mt-2 h-4 w-48" />
          </div>
        </div>
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {[1, 2, 3].map((i) => (
            <Skeleton key={i} className="h-40 w-full" />
          ))}
        </div>
      </div>
    );
  }

  return (
    <>
      <div className="container mx-auto max-w-5xl p-6">
        <div className="mb-8 flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold tracking-tight">Your Sites</h1>
            <p className="text-muted-foreground">
              Manage your sites and their content
            </p>
          </div>
          <Button onClick={() => setCreateOpen(true)} className="gap-2">
            <Plus className="size-4" />
            Create Site
          </Button>
        </div>

        {sites && sites.length > 0 ? (
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {sites.map((site) => (
              <SiteCard key={site.id} site={site} />
            ))}
          </div>
        ) : (
          <div className="flex flex-col items-center justify-center rounded-lg border border-dashed py-16">
            <div className="flex size-16 items-center justify-center rounded-full bg-muted">
              <Globe className="size-8 text-muted-foreground" />
            </div>
            <h3 className="mt-4 text-lg font-semibold">No sites yet</h3>
            <p className="mt-1 text-center text-muted-foreground">
              Create your first site to start managing content
            </p>
            <Button onClick={() => setCreateOpen(true)} className="mt-6 gap-2">
              <Plus className="size-4" />
              Create Site
            </Button>
          </div>
        )}
      </div>

      <CreateSiteDialog open={createOpen} onOpenChange={setCreateOpen} />
    </>
  );
}
