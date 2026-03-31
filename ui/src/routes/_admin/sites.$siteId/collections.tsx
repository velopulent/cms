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
  { value: "media", label: "File Upload" },
];

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
        <Sheet open={createOpen} onOpenChange={setCreateOpen}>
          <SheetTrigger render={<Button />}>
            <Plus data-icon="inline-start" />
            New Collection
          </SheetTrigger>
          <SheetContent
            className={"data-[side=right]:w-full data-[side=right]:sm:max-w-xl"}
          >
            <SheetHeader>
              <SheetTitle>Create Collection</SheetTitle>
              <SheetDescription>
                Define a new collection with custom fields.
              </SheetDescription>
            </SheetHeader>
            <div className="flex-1 overflow-y-auto px-4">
              <CollectionForm
                onSubmit={(data) => {
                  createCollection(siteId, data)
                    .then(() => {
                      queryClient.invalidateQueries({
                        queryKey: ["collections", siteId],
                      });
                      setCreateOpen(false);
                      toast.success("Collection created");
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
                Create Collection
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
              Create your first collection to get started.
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
                      <Badge variant="outline">{c.slug}</Badge>
                    </TableCell>
                    <TableCell>{fieldCount} fields</TableCell>
                    <TableCell className="text-right">
                      <div className="flex justify-end gap-1">
                        <Sheet
                          open={editCollection?.id === c.id}
                          onOpenChange={(open) =>
                            setEditCollection(open ? c : null)
                          }
                        >
                          <SheetTrigger
                            render={<Button variant="ghost" size="icon" />}
                          >
                            <Pencil />
                          </SheetTrigger>
                          <SheetContent
                            className={
                              "data-[side=right]:w-full data-[side=right]:sm:max-w-xl"
                            }
                          >
                            <SheetHeader>
                              <SheetTitle>Edit Collection</SheetTitle>
                              <SheetDescription>
                                Update the collection definition.
                              </SheetDescription>
                            </SheetHeader>
                            <div className="flex-1 overflow-y-auto px-4">
                              <CollectionForm
                                initialData={c}
                                onSubmit={(data) => {
                                  updateCollection(siteId, c.slug, {
                                    name: data.name,
                                    slug: data.slug,
                                    definition: data.definition,
                                  })
                                    .then(() => {
                                      queryClient.invalidateQueries({
                                        queryKey: ["collections", siteId],
                                      });
                                      setEditCollection(null);
                                      toast.success("Collection updated");
                                    })
                                    .catch((err: Error) =>
                                      toast.error(err.message),
                                    );
                                }}
                              />
                            </div>
                            <SheetFooter>
                              <Button
                                type="submit"
                                form="collection-form-edit"
                                disabled={false}
                              >
                                Update Collection
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
          <Input
            placeholder="Field name"
            value={field.name}
            onChange={(e) =>
              form.setFieldValue(`fields[${index}].name`, e.target.value)
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
  _id: z.string(),
});

const collectionFormSchema = z.object({
  name: z.string().min(1, "Collection name is required"),
  slug: z.string().min(1, "Slug is required"),
  fields: z.array(collectionFieldSchema).min(1, "Add at least one field"),
});

type CollectionFormValues = {
  name: string;
  slug: string;
  fields: ContentFieldWithId[];
};

function CollectionForm({
  initialData,
  onSubmit,
}: {
  initialData?: Collection;
  onSubmit: (data: {
    name: string;
    slug: string;
    definition: SchemaDefinition;
  }) => void;
}) {
  const [slugManuallyEdited, setSlugManuallyEdited] = useState(!!initialData);

  const initialFields: ContentFieldWithId[] = (() => {
    if (initialData) {
      try {
        const def: SchemaDefinition = JSON.parse(initialData.definition);
        return (def.fields ?? []).map((f, i) => ({
          ...f,
          _id: `init-${i}`,
        }));
      } catch {
    return [{ name: "", type: "text", required: false, _id: "default-0" }];
      }
    }
    return [];
  })();

  const form = useForm({
    defaultValues: {
      name: initialData?.name ?? "",
      slug: initialData?.slug ?? "",
      fields: initialFields,
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
                <p className="text-sm text-muted-foreground">
                  No fields defined. Add at least one field.
                </p>
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
