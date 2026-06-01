import type { DragEndEvent } from "@dnd-kit/core";
import {
  closestCenter,
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { GripVertical, Layers, Pencil, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { z } from "zod";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Field,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Sheet,
  SheetClose,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet";
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
  type Collection,
  type ContentField,
  createCollection,
  deleteCollection,
  getCollections,
  type SchemaDefinition,
  updateCollection,
} from "@/lib/api";

interface ContentFieldWithId extends ContentField {
  _id: string;
}

export const Route = createFileRoute("/_admin/sites/$siteId/collections")({
  component: CollectionsPage,
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
  { value: "image", label: "Image" },
  { value: "video", label: "Video" },
  { value: "audio", label: "Audio" },
  { value: "document", label: "Document" },
  { value: "archive", label: "Archive" },
];

const FILE_FIELD_TYPES = ["image", "video", "audio", "document", "archive"];

const CONTENT_TYPE_MIME_TYPES: Record<string, string[]> = {
  image: [
    "image/jpeg",
    "image/png",
    "image/gif",
    "image/webp",
    "image/avif",
    "image/svg+xml",
  ],
  video: ["video/mp4", "video/webm", "video/ogg", "video/quicktime"],
  audio: ["audio/mpeg", "audio/wav", "audio/ogg", "audio/webm", "audio/aac"],
  document: [
    "application/pdf",
    "application/msword",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.ms-excel",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "text/plain",
    "text/csv",
    "text/markdown",
  ],
  archive: [
    "application/zip",
    "application/gzip",
    "application/x-tar",
    "application/x-7z-compressed",
  ],
};

function slugify(text: string) {
  return text
    .toLowerCase()
    .trim()
    .replace(/[^\w\s-]/g, "")
    .replace(/[\s_]+/g, "-")
    .replace(/-+/g, "-");
}

function CollectionsPage() {
  const { siteId } = Route.useParams();
  const queryClient = useQueryClient();
  const [createOpen, setCreateOpen] = useState(false);
  const [editCollection, setEditCollection] = useState<Collection | null>(null);
  const [pendingIsSingleton, setPendingIsSingleton] = useState(false);

  const { data: collections, isLoading } = useQuery({
    queryKey: ["collections", siteId],
    queryFn: () => getCollections(siteId),
  });

  const deleteMutation = useMutation({
    mutationFn: (slug: string) => deleteCollection(siteId, slug),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["collections", siteId] });
      toast.success("Collection deleted");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  return (
    <div className="flex flex-col gap-6 p-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold">Collections</h1>
          <p className="text-sm text-muted-foreground">
            Define the structure of your content
          </p>
        </div>
        <Sheet
          open={createOpen}
          onOpenChange={(open) => {
            setCreateOpen(open);
            if (!open) setPendingIsSingleton(false);
          }}
        >
          <SheetTrigger render={<Button />}>
            <Plus data-icon="inline-start" />
            New
          </SheetTrigger>
          <SheetContent
            className={"data-[side=right]:w-full data-[side=right]:sm:max-w-xl"}
          >
            <SheetHeader>
              <SheetTitle>
                {pendingIsSingleton ? "Create Singleton" : "Create Collection"}
              </SheetTitle>
              <SheetDescription>
                {pendingIsSingleton
                  ? "Define a new singleton with custom fields."
                  : "Define a new collection with custom fields."}
              </SheetDescription>
            </SheetHeader>
            <div className="flex-1 overflow-y-auto px-4">
              <CollectionForm
                onIsSingletonChange={setPendingIsSingleton}
                onSubmit={(data) => {
                  createCollection(siteId, data)
                    .then(() => {
                      queryClient.invalidateQueries({
                        queryKey: ["collections", siteId],
                      });
                      setCreateOpen(false);
                      setPendingIsSingleton(false);
                      toast.success(
                        data.is_singleton
                          ? "Singleton created"
                          : "Collection created",
                      );
                    })
                    .catch((err: Error) => toast.error(err.message));
                }}
              />
            </div>
            <SheetFooter>
              <Button
                type="submit"
                form="collection-form-create"
                disabled={false}
              >
                {pendingIsSingleton ? "Create Singleton" : "Create Collection"}
              </Button>
              <SheetClose
                render={
                  <Button type="button" variant="outline">
                    Cancel
                  </Button>
                }
              />
            </SheetFooter>
          </SheetContent>
        </Sheet>
      </div>

      {isLoading ? (
        <div className="flex flex-col gap-2">
          <Skeleton className="h-12 w-full" />
          <Skeleton className="h-12 w-full" />
        </div>
      ) : !collections?.length ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Layers className="mb-4 size-10 text-muted-foreground" />
            <p className="text-lg font-medium">No collections yet</p>
            <p className="text-sm text-muted-foreground">
              Create your first collection or singleton to get started.
            </p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Type</TableHead>
                <TableHead>Slug</TableHead>
                <TableHead>Fields</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {collections.map((c: Collection) => {
                let fieldCount = 0;
                try {
                  const def: SchemaDefinition = JSON.parse(c.definition);
                  fieldCount = def.fields?.length ?? 0;
                } catch {
                  // invalid json
                }
                return (
                  <TableRow key={c.id}>
                    <TableCell className="font-medium">{c.name}</TableCell>
                    <TableCell>
                      <Badge variant={c.is_singleton ? "secondary" : "outline"}>
                        {c.is_singleton ? "Singleton" : "Collection"}
                      </Badge>
                    </TableCell>
                    <TableCell>
                      <Badge variant="outline">{c.slug}</Badge>
                    </TableCell>
                    <TableCell>{fieldCount} fields</TableCell>
                    <TableCell className="text-right">
                      <div className="flex justify-end gap-1">
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => setEditCollection(c)}
                        >
                          <Pencil />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => deleteMutation.mutate(c.slug)}
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

      <Sheet
        open={editCollection !== null}
        onOpenChange={(open) => !open && setEditCollection(null)}
      >
        <SheetContent
          className={"data-[side=right]:w-full data-[side=right]:sm:max-w-xl"}
        >
          <SheetHeader>
            <SheetTitle>
              {editCollection?.is_singleton
                ? "Edit Singleton"
                : "Edit Collection"}
            </SheetTitle>
            <SheetDescription>
              {editCollection?.is_singleton
                ? "Update the singleton definition."
                : "Update the collection definition."}
            </SheetDescription>
          </SheetHeader>
          <div className="flex-1 overflow-y-auto px-4">
            {editCollection && (
              <CollectionForm
                key={editCollection.id}
                initialData={editCollection}
                onSubmit={(data) => {
                  updateCollection(siteId, editCollection.slug, {
                    name: data.name,
                    slug: data.slug,
                    definition: data.definition,
                  })
                    .then(() => {
                      queryClient.invalidateQueries({
                        queryKey: ["collections", siteId],
                      });
                      const wasSingleton = editCollection.is_singleton;
                      setEditCollection(null);
                      toast.success(
                        wasSingleton
                          ? "Singleton updated"
                          : "Collection updated",
                      );
                    })
                    .catch((err: Error) => toast.error(err.message));
                }}
              />
            )}
          </div>
          <SheetFooter>
            <Button type="submit" form="collection-form-edit" disabled={false}>
              {editCollection?.is_singleton
                ? "Update Singleton"
                : "Update Collection"}
            </Button>
            <SheetClose
              render={
                <Button type="button" variant="outline">
                  Cancel
                </Button>
              }
            />
          </SheetFooter>
        </SheetContent>
      </Sheet>
    </div>
  );
}

// --- Sortable Field Item ---

function SortableFieldItem({
  field,
  index,
  form,
  removeField,
}: {
  field: ContentFieldWithId;
  index: number;
  // biome-ignore lint/suspicious/noExplicitAny: TanStack Form instance with complex generics
  form: any;
  removeField: (index: number) => void;
}) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: field._id });

  const [nameTouched, setNameTouched] = useState(false);
  const nameInvalid = nameTouched && !field.name.trim();

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
  };

  return (
    <div
      ref={setNodeRef}
      style={style}
      className="flex items-start gap-2 rounded-lg border p-3"
    >
      <button
        type="button"
        className="mt-2 cursor-grab text-muted-foreground hover:text-foreground"
        {...attributes}
        {...listeners}
      >
        <GripVertical className="size-4" />
      </button>
      <div className="flex flex-1 flex-col gap-2">
        <div className="flex gap-2">
          <Field data-invalid={nameInvalid} className="flex-1">
            <Input
              placeholder="Field name"
              value={field.name}
              onBlur={() => setNameTouched(true)}
              onChange={(e) =>
                form.setFieldValue(`fields[${index}].name`, e.target.value)
              }
              aria-invalid={nameInvalid}
            />
            {nameInvalid && (
              <FieldError errors={[{ message: "Field name is required" }]} />
            )}
          </Field>
          <Select
            items={[
              ...FIELD_TYPES.map((ft) => ({
                label: ft.label,
                value: ft.value,
              })),
            ]}
            value={field.type}
            onValueChange={(val) =>
              form.setFieldValue(`fields[${index}].type`, val as string)
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
        <Field orientation="horizontal">
          <Checkbox
            id={`required-${field._id}`}
            checked={field.required ?? false}
            onCheckedChange={(checked) =>
              form.setFieldValue(`fields[${index}].required`, !!checked)
            }
          />
          <FieldLabel
            htmlFor={`required-${field._id}`}
            className="font-normal text-xs text-muted-foreground"
          >
            Required
          </FieldLabel>
        </Field>
        {field.type === "select" && (
          <div className="flex flex-col gap-2 mt-2">
            <FieldLabel className="text-xs">Options</FieldLabel>
            <div className="flex flex-wrap gap-1">
              {(field.options ?? []).map((opt, optIdx) => (
                <span
                  key={optIdx}
                  className="inline-flex items-center gap-1 rounded-md border border-border bg-background px-2 py-0.5 text-xs"
                >
                  {opt}
                  <button
                    type="button"
                    className="ml-0.5 text-muted-foreground hover:text-foreground"
                    onClick={() => {
                      const newOpts = (field.options ?? []).filter(
                        (_, i) => i !== optIdx,
                      );
                      form.setFieldValue(
                        `fields[${index}].options`,
                        newOpts,
                      );
                    }}
                  >
                    ×
                  </button>
                </span>
              ))}
            </div>
            <Input
              placeholder="Add option and press Enter"
              className="h-8 text-xs"
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  const input = e.target as HTMLInputElement;
                  const val = input.value.trim();
                  if (val) {
                    const current = field.options ?? [];
                    form.setFieldValue(`fields[${index}].options`, [
                      ...current,
                      val,
                    ]);
                    input.value = "";
                  }
                }
              }}
            />
          </div>
        )}
        {FILE_FIELD_TYPES.includes(field.type) && (
          <div className="flex flex-col gap-2 mt-2">
            <FieldLabel className="text-xs">
              Accepted MIME types{" "}
              <span className="text-muted-foreground">(optional)</span>
            </FieldLabel>
            <div className="flex flex-wrap gap-1">
              {(CONTENT_TYPE_MIME_TYPES[field.type] ?? []).map((mime) => {
                const selected = (field.accept ?? []).includes(mime);
                return (
                  <button
                    key={mime}
                    type="button"
                    className={`inline-flex items-center rounded-md border px-2 py-0.5 text-xs transition-colors ${
                      selected
                        ? "border-primary bg-primary/10 text-primary"
                        : "border-border bg-background text-muted-foreground hover:text-foreground"
                    }`}
                    onClick={() => {
                      const current = field.accept ?? [];
                      const newAccept = selected
                        ? current.filter((a) => a !== mime)
                        : [...current, mime];
                      form.setFieldValue(
                        `fields[${index}].accept`,
                        newAccept.length > 0 ? newAccept : undefined,
                      );
                    }}
                  >
                    {mime.split("/")[1] ?? mime}
                  </button>
                );
              })}
            </div>
            <p className="text-xs text-muted-foreground">
              {(() => {
                const acceptCount = (field.accept ?? []).length;
                if (acceptCount === 0)
                  return "All files in this category accepted";
                return `${acceptCount} type${acceptCount === 1 ? "" : "s"} selected`;
              })()}
            </p>
          </div>
        )}
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
  );
}

// --- Collection Form ---

const collectionFieldSchema = z.object({
  name: z.string().min(1, "Field name is required"),
  type: z.string(),
  required: z.boolean().optional(),
  options: z.array(z.string()).optional(),
  accept: z.array(z.string()).optional(),
  _id: z.string(),
});

const collectionFormSchema = z.object({
  name: z.string().min(1, "Collection name is required"),
  slug: z.string().min(1, "Slug is required"),
  fields: z.array(collectionFieldSchema).min(1, "Add at least one field"),
  is_singleton: z.boolean(),
});

type CollectionFormValues = {
  name: string;
  slug: string;
  fields: ContentFieldWithId[];
  is_singleton: boolean;
};

function CollectionForm({
  initialData,
  onSubmit,
  onIsSingletonChange,
}: {
  initialData?: Collection;
  onSubmit: (data: {
    name: string;
    slug: string;
    definition: SchemaDefinition;
    is_singleton?: boolean;
  }) => void;
  onIsSingletonChange?: (isSingleton: boolean) => void;
}) {
  const [slugManuallyEdited, setSlugManuallyEdited] = useState(!!initialData);
  const isEdit = !!initialData;

  const initialFields: ContentFieldWithId[] = (() => {
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
    return [{ name: "", type: "text", required: false, _id: "default-0" }];
  })();

  const form = useForm({
    defaultValues: {
      name: initialData?.name ?? "",
      slug: initialData?.slug ?? "",
      fields: initialFields,
      is_singleton: initialData?.is_singleton ?? false,
    } as CollectionFormValues,
    validators: {
      onSubmit: collectionFormSchema,
    },
    onSubmit: async ({ value }) => {
      const cleanFields: ContentField[] = value.fields.map(
        ({ _id, ...rest }) => rest,
      );
      onSubmit({
        name: value.name,
        slug: value.slug,
        definition: { fields: cleanFields },
        is_singleton: value.is_singleton,
      });
    },
  });

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 8,
      },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (over && active.id !== over.id) {
      const currentFields = form.getFieldValue("fields");
      const oldIndex = currentFields.findIndex((f) => f._id === active.id);
      const newIndex = currentFields.findIndex((f) => f._id === over.id);
      if (oldIndex !== -1 && newIndex !== -1) {
        form.setFieldValue(
          "fields",
          arrayMove(currentFields, oldIndex, newIndex),
        );
      }
    }
  };

  const addField = () => {
    form.pushFieldValue("fields", {
      name: "",
      type: "text",
      required: false,
      _id: `new-${Date.now()}-${form.getFieldValue("fields").length}`,
    });
  };

  const removeField = (index: number) => {
    form.removeFieldValue("fields", index);
  };

  const formId = initialData
    ? `collection-form-edit`
    : `collection-form-create`;

  return (
    <form
      id={formId}
      onSubmit={(e) => {
        e.preventDefault();
        form.handleSubmit();
      }}
      className="flex flex-col gap-4 pb-4"
    >
      <FieldGroup>
        <form.Field
          name="name"
          children={(field) => {
            const isInvalid =
              field.state.meta.isTouched && !field.state.meta.isValid;
            return (
              <Field data-invalid={isInvalid}>
                <FieldLabel htmlFor={field.name}>Name</FieldLabel>
                <Input
                  id={field.name}
                  placeholder="e.g. Blog Post"
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(e) => {
                    field.handleChange(e.target.value);
                    if (!slugManuallyEdited) {
                      form.setFieldValue("slug", slugify(e.target.value));
                    }
                  }}
                  aria-invalid={isInvalid}
                />
                {isInvalid && <FieldError errors={field.state.meta.errors} />}
              </Field>
            );
          }}
        />
        <form.Field
          name="slug"
          children={(field) => {
            const isInvalid =
              field.state.meta.isTouched && !field.state.meta.isValid;
            return (
              <Field data-invalid={isInvalid}>
                <FieldLabel htmlFor={field.name}>Slug</FieldLabel>
                <Input
                  id={field.name}
                  placeholder="e.g. blog-post"
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(e) => {
                    field.handleChange(e.target.value);
                    setSlugManuallyEdited(true);
                  }}
                  aria-invalid={isInvalid}
                />
                {isInvalid && <FieldError errors={field.state.meta.errors} />}
              </Field>
            );
          }}
        />
      </FieldGroup>

      {!isEdit && (
        <form.Field
          name="is_singleton"
          children={(field) => (
            <Field>
              <FieldLabel>Type</FieldLabel>
              <Select
                items={[
                  { label: "Collection (multiple entries)", value: "false" },
                  { label: "Singleton (single entry)", value: "true" },
                ]}
                value={field.state.value ? "true" : "false"}
                onValueChange={(val) => {
                  const next = val === "true";
                  field.handleChange(next);
                  onIsSingletonChange?.(next);
                }}
              >
                <SelectTrigger className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectItem value="false">
                      Collection (multiple entries)
                    </SelectItem>
                    <SelectItem value="true">
                      Singleton (single entry)
                    </SelectItem>
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>
          )}
        />
      )}

      <form.Field
        name="fields"
        children={(arrayField) => {
          const fields = arrayField.state.value;
          return (
            <div className="flex flex-col gap-2">
              <div className="flex items-center justify-between">
                <p className="text-sm font-medium">Fields</p>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={addField}
                >
                  <Plus data-icon="inline-start" />
                  Add Field
                </Button>
              </div>
              {fields.length === 0 && (
                <Field data-invalid={!arrayField.state.meta.isValid}>
                  <p className="text-sm text-muted-foreground">
                    No fields defined yet.
                  </p>
                  {!arrayField.state.meta.isValid && (
                    <FieldError errors={arrayField.state.meta.errors} />
                  )}
                </Field>
              )}
              <DndContext
                sensors={sensors}
                collisionDetection={closestCenter}
                onDragEnd={handleDragEnd}
              >
                <SortableContext
                  items={fields.map((f) => f._id)}
                  strategy={verticalListSortingStrategy}
                >
                  <div className="flex flex-col gap-3">
                    {fields.map((field, index) => (
                      <SortableFieldItem
                        key={field._id}
                        field={field}
                        index={index}
                        form={form}
                        removeField={removeField}
                      />
                    ))}
                  </div>
                </SortableContext>
              </DndContext>
            </div>
          );
        }}
      />
    </form>
  );
}
