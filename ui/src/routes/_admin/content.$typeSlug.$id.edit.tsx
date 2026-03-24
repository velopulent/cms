import { useMutation, useQuery } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { DynamicForm } from "@/components/dynamic-form";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import {
  type ContentTypeSchema,
  getContentById,
  getContentType,
  updateContent,
} from "@/lib/api";

export const Route = createFileRoute("/_admin/content/$typeSlug/$id/edit")({
  component: EditContentPage,
});

function EditContentPage() {
  const { typeSlug, id } = Route.useParams();
  const navigate = useNavigate();
  const [data, setData] = useState<Record<string, unknown>>({});
  const [slug, setSlug] = useState("");
  const [initialized, setInitialized] = useState(false);

  const { data: contentType, isLoading: typeLoading } = useQuery({
    queryKey: ["content-type", typeSlug],
    queryFn: () => getContentType(typeSlug),
  });

  const { data: content, isLoading: contentLoading } = useQuery({
    queryKey: ["content", id],
    queryFn: () => getContentById(id),
    enabled: !!id,
  });

  useEffect(() => {
    if (content && !initialized) {
      try {
        setData(JSON.parse(content.data));
      } catch {
        setData({});
      }
      setSlug(content.slug);
      setInitialized(true);
    }
  }, [content, initialized]);

  const updateMutation = useMutation({
    mutationFn: () =>
      updateContent(id, {
        data,
        slug,
      }),
    onSuccess: () => {
      toast.success("Content updated");
      navigate({ to: "/content/$typeSlug", params: { typeSlug } });
    },
    onError: (err: Error) => toast.error(err.message),
  });

  let schema: ContentTypeSchema | null = null;
  if (contentType) {
    try {
      schema = JSON.parse(contentType.schema_json);
    } catch {
      // invalid schema
    }
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!slug.trim()) return;
    updateMutation.mutate();
  };

  const isLoading = typeLoading || contentLoading;

  if (isLoading || !initialized) {
    return (
      <div className="flex flex-col gap-6 p-6">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-64 w-full" />
      </div>
    );
  }

  if (!contentType || !content) {
    return (
      <div className="p-6">
        <p>Content not found.</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6 p-6">
      <div className="flex items-center gap-3">
        <Button
          variant="ghost"
          size="icon"
          render={<Link to="/content/$typeSlug" params={{ typeSlug }} />}
        >
          <ArrowLeft />
        </Button>
        <div>
          <div className="flex items-center gap-2">
            <h1 className="text-2xl font-semibold">Edit {contentType.name}</h1>
            <Badge
              variant={content.status === "published" ? "default" : "secondary"}
            >
              {content.status}
            </Badge>
          </div>
          <p className="text-sm text-muted-foreground">
            Edit {contentType.name.toLowerCase()} #{content.id.slice(0, 8)}
          </p>
        </div>
      </div>

      <form onSubmit={handleSubmit} className="flex flex-col gap-6">
        <Card>
          <CardHeader>
            <CardTitle>Content</CardTitle>
          </CardHeader>
          <CardContent>
            {schema ? (
              <DynamicForm
                fields={schema.fields}
                values={data}
                onChange={setData}
              />
            ) : (
              <p className="text-sm text-muted-foreground">
                Invalid content type schema.
              </p>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Settings</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="flex flex-col gap-2">
              <label htmlFor="edit-slug" className="text-sm font-medium">
                Slug
              </label>
              <Input
                id="edit-slug"
                value={slug}
                onChange={(e) => setSlug(e.target.value)}
              />
            </div>
          </CardContent>
        </Card>

        <div className="flex gap-2">
          <Button
            type="submit"
            disabled={updateMutation.isPending || !slug.trim()}
          >
            {updateMutation.isPending ? "Saving..." : "Save"}
          </Button>
          <Button
            type="button"
            variant="outline"
            render={<Link to="/content/$typeSlug" params={{ typeSlug }} />}
          >
            Cancel
          </Button>
        </div>
      </form>
    </div>
  );
}
