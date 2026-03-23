import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { GripVertical, Layers, Pencil, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  type ContentField,
  type ContentType,
  type ContentTypeSchema,
  createContentType,
  deleteContentType,
  getContentTypes,
  updateContentType,
} from "@/lib/api";

interface ContentFieldWithId extends ContentField {
  _id: string;
}

export const Route = createFileRoute("/_admin/content-types")({
  component: ContentTypesPage,
});

const FIELD_TYPES = [
  { value: "text", label: "Text" },
  { value: "textarea", label: "Text Area" },
  { value: "rich_text", label: "Rich Text (Tiptap)" },
  { value: "number", label: "Number" },
  { value: "boolean", label: "Boolean" },
  { value: "date", label: "Date" },
  { value: "select", label: "Select" },
  { value: "image_url", label: "Image URL" },
];

function slugify(text: string) {
  return text
    .toLowerCase()
    .trim()
    .replace(/[^\w\s-]/g, "")
    .replace(/[\s_]+/g, "-")
    .replace(/-+/g, "-");
}

function ContentTypesPage() {
  const queryClient = useQueryClient();
  const [createOpen, setCreateOpen] = useState(false);
  const [editType, setEditType] = useState<ContentType | null>(null);

  const { data: contentTypes, isLoading } = useQuery({
    queryKey: ["content-types"],
    queryFn: getContentTypes,
  });

  const deleteMutation = useMutation({
    mutationFn: deleteContentType,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["content-types"] });
      toast.success("Content type deleted");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  return (
    <div className="flex flex-col gap-6 p-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold">Content Types</h1>
          <p className="text-sm text-muted-foreground">
            Define the structure of your content
          </p>
        </div>
        <Dialog open={createOpen} onOpenChange={setCreateOpen}>
          <DialogTrigger render={<Button />}>
            <Plus data-icon="inline-start" />
            New Type
          </DialogTrigger>
          <DialogContent className="max-h-[80vh] overflow-y-auto sm:max-w-lg">
            <DialogHeader>
              <DialogTitle>Create Content Type</DialogTitle>
              <DialogDescription>
                Define a new content type with custom fields.
              </DialogDescription>
            </DialogHeader>
            <ContentTypeForm
              onSubmit={(data) => {
                createContentType(data)
                  .then(() => {
                    queryClient.invalidateQueries({
                      queryKey: ["content-types"],
                    });
                    setCreateOpen(false);
                    toast.success("Content type created");
                  })
                  .catch((err: Error) => toast.error(err.message));
              }}
            />
          </DialogContent>
        </Dialog>
      </div>

      {isLoading ? (
        <div className="flex flex-col gap-2">
          <Skeleton className="h-12 w-full" />
          <Skeleton className="h-12 w-full" />
        </div>
      ) : !contentTypes?.length ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Layers className="mb-4 size-10 text-muted-foreground" />
            <p className="text-lg font-medium">No content types yet</p>
            <p className="text-sm text-muted-foreground">
              Create your first content type to get started.
            </p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Slug</TableHead>
                <TableHead>Fields</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {contentTypes.map((ct: ContentType) => {
                let fieldCount = 0;
                try {
                  const schema: ContentTypeSchema = JSON.parse(ct.schema_json);
                  fieldCount = schema.fields?.length ?? 0;
                } catch {
                  // invalid json
                }
                return (
                  <TableRow key={ct.id}>
                    <TableCell className="font-medium">{ct.name}</TableCell>
                    <TableCell>
                      <Badge variant="outline">{ct.slug}</Badge>
                    </TableCell>
                    <TableCell>{fieldCount} fields</TableCell>
                    <TableCell className="text-right">
                      <div className="flex justify-end gap-1">
                        <Dialog
                          open={editType?.id === ct.id}
                          onOpenChange={(open) => setEditType(open ? ct : null)}
                        >
                          <DialogTrigger
                            render={<Button variant="ghost" size="icon" />}
                          >
                            <Pencil />
                          </DialogTrigger>
                          <DialogContent className="max-h-[80vh] overflow-y-auto sm:max-w-lg">
                            <DialogHeader>
                              <DialogTitle>Edit Content Type</DialogTitle>
                              <DialogDescription>
                                Update the content type definition.
                              </DialogDescription>
                            </DialogHeader>
                            <ContentTypeForm
                              initialData={ct}
                              onSubmit={(data) => {
                                updateContentType(ct.slug, {
                                  name: data.name,
                                  slug: data.slug,
                                  schema_json: data.schema_json,
                                })
                                  .then(() => {
                                    queryClient.invalidateQueries({
                                      queryKey: ["content-types"],
                                    });
                                    setEditType(null);
                                    toast.success("Content type updated");
                                  })
                                  .catch((err: Error) =>
                                    toast.error(err.message),
                                  );
                              }}
                            />
                          </DialogContent>
                        </Dialog>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => deleteMutation.mutate(ct.slug)}
                          disabled={deleteMutation.isPending}
                        >
                          <Trash2 />
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </Card>
      )}
    </div>
  );
}

// --- Content Type Form ---

function ContentTypeForm({
  initialData,
  onSubmit,
}: {
  initialData?: ContentType;
  onSubmit: (data: {
    name: string;
    slug: string;
    schema_json: ContentTypeSchema;
  }) => void;
}) {
  const [name, setName] = useState(initialData?.name ?? "");
  const [slug, setSlug] = useState(initialData?.slug ?? "");
  const [fields, setFields] = useState<ContentFieldWithId[]>(() => {
    if (initialData) {
      try {
        const schema: ContentTypeSchema = JSON.parse(initialData.schema_json);
        return (schema.fields ?? []).map((f, i) => ({
          ...f,
          _id: `init-${i}`,
        }));
      } catch {
        return [];
      }
    }
    return [];
  });

  const [slugManuallyEdited, setSlugManuallyEdited] = useState(!!initialData);

  const handleNameChange = (value: string) => {
    setName(value);
    if (!slugManuallyEdited) {
      setSlug(slugify(value));
    }
  };

  const addField = () => {
    setFields([
      ...fields,
      {
        name: "",
        type: "text",
        required: false,
        _id: `new-${Date.now()}-${fields.length}`,
      },
    ]);
  };

  const removeField = (index: number) => {
    setFields(fields.filter((_, i) => i !== index));
  };

  const updateField = (index: number, updates: Partial<ContentField>) => {
    setFields(fields.map((f, i) => (i === index ? { ...f, ...updates } : f)));
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !slug.trim()) return;
    const cleanFields: ContentField[] = fields.map(({ _id, ...rest }) => rest);
    onSubmit({
      name,
      slug,
      schema_json: { fields: cleanFields },
    });
  };

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-4">
      <div className="flex flex-col gap-2">
        <label htmlFor="ct-name" className="text-sm font-medium">
          Name
        </label>
        <Input
          id="ct-name"
          placeholder="e.g. Blog Post"
          value={name}
          onChange={(e) => handleNameChange(e.target.value)}
        />
      </div>
      <div className="flex flex-col gap-2">
        <label htmlFor="ct-slug" className="text-sm font-medium">
          Slug
        </label>
        <Input
          id="ct-slug"
          placeholder="e.g. blog-post"
          value={slug}
          onChange={(e) => {
            setSlug(e.target.value);
            setSlugManuallyEdited(true);
          }}
        />
      </div>

      <div className="flex flex-col gap-2">
        <div className="flex items-center justify-between">
          <p className="text-sm font-medium">Fields</p>
          <Button type="button" variant="outline" size="sm" onClick={addField}>
            <Plus data-icon="inline-start" />
            Add Field
          </Button>
        </div>
        {fields.length === 0 && (
          <p className="text-sm text-muted-foreground">
            No fields defined. Add at least one field.
          </p>
        )}
        <div className="flex flex-col gap-3">
          {fields.map((field, index) => (
            <div
              key={field._id}
              className="flex items-start gap-2 rounded-lg border p-3"
            >
              <GripVertical className="mt-2 size-4 shrink-0 text-muted-foreground" />
              <div className="flex flex-1 flex-col gap-2">
                <div className="flex gap-2">
                  <Input
                    placeholder="Field name"
                    value={field.name}
                    onChange={(e) =>
                      updateField(index, { name: e.target.value })
                    }
                    className="flex-1"
                  />
                  <Select
                    items={[
                      ...FIELD_TYPES.map((ft) => ({
                        label: ft.label,
                        value: ft.value,
                      })),
                    ]}
                    value={field.type}
                    onValueChange={(val) =>
                      updateField(index, { type: val as string })
                    }
                  >
                    <SelectTrigger className="w-[160px]">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        {FIELD_TYPES.map((ft) => (
                          <SelectItem key={ft.value} value={ft.value}>
                            {ft.label}
                          </SelectItem>
                        ))}
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </div>
                <label className="flex items-center gap-2 text-xs text-muted-foreground">
                  <input
                    type="checkbox"
                    id={`required-${field._id}`}
                    checked={field.required ?? false}
                    onChange={(e) =>
                      updateField(index, { required: e.target.checked })
                    }
                  />
                  Required
                </label>
              </div>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                onClick={() => removeField(index)}
              >
                <Trash2 />
              </Button>
            </div>
          ))}
        </div>
      </div>

      <DialogFooter>
        <DialogClose render={<Button type="button" variant="outline" />}>
          Cancel
        </DialogClose>
        <Button type="submit" disabled={!name.trim() || !slug.trim()}>
          {initialData ? "Update" : "Create"}
        </Button>
      </DialogFooter>
    </form>
  );
}
