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
  type Schema,
  type SchemaDefinition,
  createSchema,
  deleteSchema,
  getSchemas,
  updateSchema,
} from "@/lib/api";

interface ContentFieldWithId extends ContentField {
  _id: string;
}

export const Route = createFileRoute("/_admin/sites/$siteId/schemas")({
  component: SchemasPage,
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

function SchemasPage() {
  const { siteId } = Route.useParams();
  const queryClient = useQueryClient();
  const [createOpen, setCreateOpen] = useState(false);
  const [editSchema, setEditSchema] = useState<Schema | null>(null);

  const { data: schemas, isLoading } = useQuery({
    queryKey: ["schemas", siteId],
    queryFn: () => getSchemas(siteId),
  });

  const deleteMutation = useMutation({
    mutationFn: (slug: string) => deleteSchema(siteId, slug),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["schemas", siteId] });
      toast.success("Schema deleted");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  return (
    <div className="flex flex-col gap-6 p-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold">Schemas</h1>
          <p className="text-sm text-muted-foreground">
            Define the structure of your content
          </p>
        </div>
        <Dialog open={createOpen} onOpenChange={setCreateOpen}>
          <DialogTrigger render={<Button />}>
            <Plus data-icon="inline-start" />
            New Schema
          </DialogTrigger>
          <DialogContent className="max-h-[80vh] overflow-y-auto sm:max-w-lg">
            <DialogHeader>
              <DialogTitle>Create Schema</DialogTitle>
              <DialogDescription>
                Define a new schema with custom fields.
              </DialogDescription>
            </DialogHeader>
            <SchemaForm
              onSubmit={(data) => {
                createSchema(siteId, data)
                  .then(() => {
                    queryClient.invalidateQueries({
                      queryKey: ["schemas", siteId],
                    });
                    setCreateOpen(false);
                    toast.success("Schema created");
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
      ) : !schemas?.length ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Layers className="mb-4 size-10 text-muted-foreground" />
            <p className="text-lg font-medium">No schemas yet</p>
            <p className="text-sm text-muted-foreground">
              Create your first schema to get started.
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
              {schemas.map((s: Schema) => {
                let fieldCount = 0;
                try {
                  const def: SchemaDefinition = JSON.parse(s.definition);
                  fieldCount = def.fields?.length ?? 0;
                } catch {
                  // invalid json
                }
                return (
                  <TableRow key={s.id}>
                    <TableCell className="font-medium">{s.name}</TableCell>
                    <TableCell>
                      <Badge variant="outline">{s.slug}</Badge>
                    </TableCell>
                    <TableCell>{fieldCount} fields</TableCell>
                    <TableCell className="text-right">
                      <div className="flex justify-end gap-1">
                        <Dialog
                          open={editSchema?.id === s.id}
                          onOpenChange={(open) => setEditSchema(open ? s : null)}
                        >
                          <DialogTrigger
                            render={<Button variant="ghost" size="icon" />}
                          >
                            <Pencil />
                          </DialogTrigger>
                          <DialogContent className="max-h-[80vh] overflow-y-auto sm:max-w-lg">
                            <DialogHeader>
                              <DialogTitle>Edit Schema</DialogTitle>
                              <DialogDescription>
                                Update the schema definition.
                              </DialogDescription>
                            </DialogHeader>
                            <SchemaForm
                              initialData={s}
                              onSubmit={(data) => {
                                updateSchema(siteId, s.slug, {
                                  name: data.name,
                                  slug: data.slug,
                                  definition: data.definition,
                                })
                                  .then(() => {
                                    queryClient.invalidateQueries({
                                      queryKey: ["schemas", siteId],
                                    });
                                    setEditSchema(null);
                                    toast.success("Schema updated");
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
                          onClick={() => deleteMutation.mutate(s.slug)}
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

// --- Schema Form ---

function SchemaForm({
  initialData,
  onSubmit,
}: {
  initialData?: Schema;
  onSubmit: (data: {
    name: string;
    slug: string;
    definition: SchemaDefinition;
  }) => void;
}) {
  const [name, setName] = useState(initialData?.name ?? "");
  const [slug, setSlug] = useState(initialData?.slug ?? "");
  const [fields, setFields] = useState<ContentFieldWithId[]>(() => {
    if (initialData) {
      try {
        const def: SchemaDefinition = JSON.parse(initialData.definition);
        return (def.fields ?? []).map((f, i) => ({
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
      definition: { fields: cleanFields },
    });
  };

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-4">
      <div className="flex flex-col gap-2">
        <label htmlFor="schema-name" className="text-sm font-medium">
          Name
        </label>
        <Input
          id="schema-name"
          placeholder="e.g. Blog Post"
          value={name}
          onChange={(e) => handleNameChange(e.target.value)}
        />
      </div>
      <div className="flex flex-col gap-2">
        <label htmlFor="schema-slug" className="text-sm font-medium">
          Slug
        </label>
        <Input
          id="schema-slug"
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
