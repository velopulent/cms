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
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { GripVertical, Layers, Pencil, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
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
  updateField,
  removeField,
}: {
  field: ContentFieldWithId;
  index: number;
  updateField: (index: number, updates: Partial<ContentField>) => void;
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
            onChange={(e) => updateField(index, { name: e.target.value })}
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
            onValueChange={(val) => updateField(index, { type: val as string })}
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
            onChange={(e) => updateField(index, { required: e.target.checked })}
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
  );
}

// --- Collection Form ---

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

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (over && active.id !== over.id) {
      setFields((items) => {
        const oldIndex = items.findIndex((i) => i._id === active.id);
        const newIndex = items.findIndex((i) => i._id === over.id);
        return arrayMove(items, oldIndex, newIndex);
      });
    }
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

  const formId = initialData
    ? `collection-form-edit`
    : `collection-form-create`;

  return (
    <form
      id={formId}
      onSubmit={handleSubmit}
      className="flex flex-col gap-4 pb-4"
    >
      <div className="flex flex-col gap-2">
        <label htmlFor="collection-name" className="text-sm font-medium">
          Name
        </label>
        <Input
          id="collection-name"
          placeholder="e.g. Blog Post"
          value={name}
          onChange={(e) => handleNameChange(e.target.value)}
        />
      </div>
      <div className="flex flex-col gap-2">
        <label htmlFor="collection-slug" className="text-sm font-medium">
          Slug
        </label>
        <Input
          id="collection-slug"
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
                  updateField={updateField}
                  removeField={removeField}
                />
              ))}
            </div>
          </SortableContext>
        </DndContext>
      </div>
    </form>
  );
}
